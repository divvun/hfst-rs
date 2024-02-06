#include "wrapper.hpp"

// Settings
static bool superblanks = false;        // Input is apertium-style superblanks (overrides blankline_separated)
static bool blankline_separated = true; // Input is separated by blank lines (as opposed to single newlines)
static bool keep_newlines = false;

// Global settings
static bool verbose = false;
static FILE *inputfile = stdin;

// TODO: default format
// TODO: tokenizer filename
static hfst::ImplementationType default_format = hfst::TROPICAL_OPENFST_TYPE;
std::string tokenizer_filename;
hfst_ol_tokenize::TokenizeSettings settings;

inline void maybe_erase_newline(string &input_text)
{
  if (!keep_newlines && input_text.size() > 0 && input_text.at(input_text.size() - 1) == '\n')
  {
    // Remove final newline
    input_text.erase(input_text.size() - 1, 1);
  }
}

inline void trim(std::string &str)
{
  while (!str.empty() && (std::isspace(str.back()) || str.back() == 0))
  {
    str.pop_back();
  }
  while (!str.empty() && (std::isspace(str.front()) || str.front() == 0))
  {
    str.erase(0, 1);
  }
}

void error(int status, int errnum, const char *fmt, ...)
{
  va_list ap;
  va_start(ap, fmt);
  //   vfprintf(stderr, fmt, ap);
  va_end(ap);
  if (errnum != 0)
  {
    fprintf(stderr, "%s", strerror(errnum));
  }
  // "\n"
  if (status != 0)
  {
    exit(status);
  }
}

ssize_t hfst_getline(char **lineptr, size_t *n, FILE *stream)
{
  errno = 0;
  ssize_t rv = -1;
  rv = getline(lineptr, n, stream);
  if ((rv < 0) && errno)
  {
    error(EXIT_FAILURE, errno, "getline failed");
  }
  return rv;
}

size_t hfst_getdelim(char **lineptr, size_t *n, int delim, FILE *stream)
{
  errno = 0;
  ssize_t rv = -1;
  rv = getdelim(lineptr, n, delim, stream);
  if ((rv < 0) && errno)
  {
    error(EXIT_FAILURE, errno, "getdelim failed");
  }
  return rv;
}

inline void process_input_0delim_print(hfst_ol::PmatchContainer &container,
                                       std::ostream &outstream,
                                       std::ostringstream &cur)
{
  const std::string &input_text{cur.str()};
  if (!input_text.empty())
  {
    match_and_print(container, outstream, input_text, settings);
  }
  cur.clear();
  cur.str(string());
}

template <bool do_superblank>
int process_input_0delim(hfst_ol::PmatchContainer &container,
                         std::ostream &outstream)
{
  char *line = NULL;
  size_t bufsize = 0;
  bool in_blank = false;
  std::ostringstream cur;
  ssize_t len = -1;
  while ((len = hfst_getdelim(&line, &bufsize, '\0', inputfile)) > 0)
  {
    bool escaped = false; // beginning of line is necessarily unescaped
    for (ssize_t i = 0; i < len; ++i)
    {
      if (escaped)
      {
        cur << line[i];
        escaped = false;
        continue;
      }
      else if (do_superblank && !in_blank && line[i] == '[')
      {
        process_input_0delim_print(container, outstream, cur);
        cur << line[i];
        in_blank = true;
      }
      else if (do_superblank && in_blank && line[i] == ']')
      {
        cur << line[i];
        if (i + 1 < len && line[i + 1] == '[')
        {
          // Join consecutive superblanks
          ++i;
          cur << line[i];
        }
        else
        {
          in_blank = false;
          print_nonmatching_sequence(cur.str(), outstream, settings);
          cur.clear();
          cur.str(string());
        }
      }
      else if (!in_blank && line[i] == '\n')
      {
        cur << line[i];
        if (verbose)
        {
          std::cout << "processing: " << cur.str() << "\\n"
                    << std::endl;
        }
        process_input_0delim_print(container, outstream, cur);
      }
      else if (line[i] == '\0')
      {
        if (verbose)
        {
          std::cout << "processing: " << cur.str() << "\\0" << std::endl;
        }
        process_input_0delim_print(container, outstream, cur);
        outstream << "<STREAMCMD:FLUSH>" << std::endl; // CG format uses this instead of \0
        outstream.flush();
        if (outstream.bad())
        {
          std::cerr << "hfst-tokenize: Could not flush file" << std::endl;
        }
      }
      else
      {
        cur << line[i];
      }
      escaped = (line[i] == '\\');
    }
    free(line);
    line = NULL;
    if (std::feof(inputfile))
    {
      break;
    }
  }
  if (in_blank)
  {
    print_nonmatching_sequence(cur.str(), outstream, settings);
  }
  else
  {
    process_input_0delim_print(container, outstream, cur);
  }
  return EXIT_SUCCESS;
}

int process_input_visl(hfst_ol::PmatchContainer &container, std::ostream &outstream)
{
  size_t bufsize = 0;
  char *buffer = 0;
  std::string line;

  ssize_t len = 0;
  while ((len = hfst_getline(&buffer, &bufsize, inputfile)) > 0)
  {
    line.assign(buffer, buffer + len);
    trim(line);
    if (!line.empty())
    {
      if (line.front() == '<' && line.back() == '>')
      {
        print_nonmatching_sequence(line, outstream, settings);
      }
      else
      {
        match_and_print(container, outstream, line, settings);
      }
    }
    else
    {
      outstream << '\n';
    }
    outstream.flush();

    buffer[0] = 0;
    len = 0;

    if (feof(inputfile))
    {
      break;
    }
  }

  if (len < 0)
  {
    len = 0;
  }

  line.assign(buffer, buffer + len);
  trim(line);
  if (!line.empty())
  {
    if (line.front() == '<' && line.back() == '>')
    {
      print_nonmatching_sequence(line, outstream, settings);
    }
    else
    {
      match_and_print(container, outstream, line, settings);
    }
  }
  outstream.flush();

  free(buffer);
  return EXIT_SUCCESS;
}

int process_input(hfst_ol::PmatchContainer &container, std::ostream &outstream)
{
  // if (settings.output_format == hfst_ol_tokenize::cg || settings.output_format == hfst_ol_tokenize::giellacg || settings.output_format == hfst_ol_tokenize::visl)
  // {
  //   outstream << std::fixed << std::setprecision(10);
  // }
  // if (settings.output_format == hfst_ol_tokenize::giellacg || superblanks)
  // {
  //   if (superblanks)
  //   {
  //     // Processing giellacg with superblanks
  //     std::cout << "Processing giellacg with superblanks" << std::endl;
  //     return process_input_0delim<true>(container, outstream);
  //   }
  //   else
  //   {
  // // Processing giellacg without superblanks
  //     std::cout << "Processing giellacg without superblanks" << std::endl;
  //     return process_input_0delim<false>(container, outstream);
  //   }
  // }
  // if (settings.output_format == hfst_ol_tokenize::visl)
  // {
  //   // Processing VISL CG 3
  //   std::cout << "Processing VISL CG 3" << std::endl;
  //   return process_input_visl(container, outstream);
  // }

  outstream << std::fixed << std::setprecision(10);
  // Processing giellacg without superblanks
  return process_input_0delim<false>(container, outstream);

  string input_text;
  char *line = NULL;
  size_t bufsize = 0;
  if (blankline_separated)
  {
    // Processing blankline separated input
    while (hfst_getline(&line, &bufsize, inputfile) > 0)
    {
      if (line[0] == '\n')
      {
        maybe_erase_newline(input_text);
        match_and_print(container, outstream, input_text, settings);
        input_text.clear();
      }
      else
      {
        input_text.append(line);
      }
      free(line);
      line = NULL;
    }
    if (!input_text.empty())
    {
      maybe_erase_newline(input_text);
      match_and_print(container, outstream, input_text, settings);
    }
  }
  else
  {
    // newline or non-separated
    // Processing non-separated input
    while (hfst_getline(&line, &bufsize, inputfile) > 0)
    {
      input_text = line;
      maybe_erase_newline(input_text);
      match_and_print(container, outstream, input_text, settings);
      free(line);
      line = NULL;
    }
  }

  return EXIT_SUCCESS;
}

hfst_ol::PmatchContainer make_naive_tokenizer(hfst::HfstTransducer *dictionary)
{
  hfst::HfstTransducer *word_boundary = hfst::pmatch::PmatchUtilityTransducers::make_latin1_whitespace_acceptor(default_format);
  hfst::HfstTransducer *punctuation = hfst::pmatch::PmatchUtilityTransducers::make_latin1_punct_acceptor(default_format);
  word_boundary->disjunct(*punctuation);
  hfst::HfstTransducer *others = hfst::pmatch::make_exc_list(word_boundary, default_format);
  others->repeat_plus();
  // make the default token less likely than any dictionary token
  others->set_final_weights(std::numeric_limits<float>::max());
  hfst::HfstTransducer *word_boundary_list = hfst::pmatch::make_list(word_boundary, default_format);
  // @BOUNDARY@ is pmatch's special input boundary marker
  word_boundary_list->disjunct(hfst::HfstTransducer("@BOUNDARY@", default_format));
  delete word_boundary;
  delete punctuation;
  hfst::HfstTransducer *left_context = new hfst::HfstTransducer(hfst::internal_epsilon, hfst::pmatch::LC_ENTRY_SYMBOL, default_format);
  hfst::HfstTransducer *right_context = new hfst::HfstTransducer(hfst::internal_epsilon, hfst::pmatch::RC_ENTRY_SYMBOL, default_format);
  left_context->concatenate(*word_boundary_list);
  right_context->concatenate(*word_boundary_list);
  delete word_boundary_list;
  hfst::HfstTransducer *left_context_exit = new hfst::HfstTransducer(hfst::internal_epsilon, hfst::pmatch::LC_EXIT_SYMBOL, default_format);
  hfst::HfstTransducer *right_context_exit = new hfst::HfstTransducer(hfst::internal_epsilon, hfst::pmatch::RC_EXIT_SYMBOL, default_format);
  left_context->concatenate(*left_context_exit);
  right_context->concatenate(*right_context_exit);
  delete left_context_exit;
  delete right_context_exit;
  std::string dict_name = dictionary->get_name();
  if (dict_name.empty())
  {
    dict_name = "unknown_pmatch_tokenized_dict";
    dictionary->set_name(dict_name);
  }
  hfst::HfstTransducer dict_ins_arc(hfst::pmatch::get_Ins_transition(dict_name.c_str()), default_format);
  // We now make the center of the tokenizer
  others->disjunct(dict_ins_arc);
  // And combine it with the context conditions
  left_context->concatenate(*others);
  left_context->concatenate(*right_context);
  delete others;
  delete right_context;
  // Because there are context conditions we need delimiter markers
  hfst::HfstTransducer *tokenizer = hfst::pmatch::add_pmatch_delimiters(left_context);
  tokenizer->set_name("TOP");
  tokenizer->minimize();
  // Convert the dictionary to olw if it wasn't already
  dictionary->convert(hfst::HFST_OLW_TYPE);
  // Get the alphabets
  std::set<std::string> dict_syms = dictionary->get_alphabet();
  std::set<std::string> tokenizer_syms = tokenizer->get_alphabet();
  std::vector<std::string> tokenizer_minus_dict;
  // What to add to the dictionary
  std::set_difference(tokenizer_syms.begin(), tokenizer_syms.end(), dict_syms.begin(), dict_syms.end(), std::inserter(tokenizer_minus_dict, tokenizer_minus_dict.begin()));
  for (std::vector<std::string>::const_iterator it = tokenizer_minus_dict.begin();
       it != tokenizer_minus_dict.end(); ++it)
  {
    dictionary->insert_to_alphabet(*it);
  }
  hfst::HfstBasicTransducer *tokenizer_basic = hfst::implementations::ConversionFunctions::
      hfst_transducer_to_hfst_basic_transducer(*tokenizer);
  hfst_ol::Transducer *tokenizer_ol = hfst::implementations::ConversionFunctions::
      hfst_basic_transducer_to_hfst_ol(tokenizer_basic,
                                       true,        // weighted
                                       "",          // no special options
                                       dictionary); // harmonize with the dictionary
  delete tokenizer_basic;
  hfst_ol::PmatchContainer retval(tokenizer_ol);
  hfst_ol::Transducer *dict_backend = hfst::implementations::ConversionFunctions::hfst_transducer_to_hfst_ol(dictionary);
  retval.add_rtn(dict_backend, dict_name);
  delete tokenizer_ol;
  return retval;
}

extern "C" int test()
{
  std::cout << "CPP TEST" << std::endl;
}

extern "C" const char* hfst_tokenize(const uint8_t *input_data)
{
  std::stringstream output;

  settings.output_format = hfst_ol_tokenize::giellacg;
  settings.print_weights = true;
  settings.print_all = true;
  settings.dedupe = true;
  settings.hack_uncompose = true;
  settings.verbose = false;
  if (settings.max_weight_classes == std::numeric_limits<int>::max())
  {
    settings.max_weight_classes = 2;
  }

  std::cout << "Reading from" << tokenizer_filename.c_str() << std::endl;
  std::ifstream instream("./gramcheck.pmhfst", std::ifstream::binary);

  if (!instream.good())
  {
    std::cerr << "Could not open file " << tokenizer_filename << std::endl;
    return "TODO";
  }

  try
  {
    std::map<std::string, std::string> first_header_attributes;
    try
    {
      first_header_attributes = hfst_ol::PmatchContainer::parse_hfst3_header(instream);
      instream.seekg(0);
      instream.clear();
    }
    catch (TransducerHeaderException &e)
    {
      std::cerr << tokenizer_filename << " doesn't look like a HFST archive. Exiting.\n"
                                         "Exception thrown:\n"
                << e.what() << std::endl;
      return "TODO";
    }

    if (first_header_attributes.count("name") == 0 || first_header_attributes["name"] != "TOP")
    {
      std::cout << "No TOP automaton found, using naive tokeniser?" << std::endl;
      std::cout << "Creating input stream" << std::endl;
      hfst::HfstInputStream is(tokenizer_filename);
      std::cout << "Creating dictionary" << std::endl;
      hfst::HfstTransducer *dictionary = new hfst::HfstTransducer(is);
      std::cout << "TEST" << std::endl;
      instream.close();
      hfst_ol::PmatchContainer container = make_naive_tokenizer(dictionary);
      delete dictionary;
      container.set_verbose(verbose);
      container.set_single_codepoint_tokenization(!settings.tokenize_multichar);
      if (process_input(container, std::cout) == EXIT_SUCCESS)
      {
        // std::cout << "OUT:" << output.str() << std::endl;
        return output.str().c_str();
      }
      else
      {
        return "TODO";
      }
    }
    else
    {
      std::cout << "TOP automaton seen, treating as pmatch script..." << std::endl;
      hfst_ol::PmatchContainer container(instream);
      container.set_verbose(verbose);
      container.set_single_codepoint_tokenization(!settings.tokenize_multichar);
      if (process_input(container, std::cout) == EXIT_SUCCESS)
      {
        // std::cout << "OUT:" << output.str() << std::endl;
        return output.str().c_str();
      }
      else
      {
        return "TODO";
      }
    }
  }
  catch (HfstException &e)
  {
    std::cerr << "Exception thrown:\n"
              << e.what() << std::endl;
    return "TODO";
  }

  return "HELLO FROM CPP!";
}
