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

int process_input(hfst_ol::PmatchContainer &container, std::ostream &outstream)
{
  outstream << std::fixed << std::setprecision(10);
  std::cout << "Processing giellacg without superblanks" << std::endl;
  // Processing giellacg without superblanks
  return process_input_0delim<false>(container, outstream);

  string input_text;
  char *line = NULL;
  size_t bufsize = 0;
  if (blankline_separated)
  {
    // Processing blankline separated input
    std::cout << "Processing blankline separated input" << std::endl;
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
    std::cout << "Processing non-separated input" << std::endl;
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

extern "C" const char *hfst_tokenize(const uint8_t *input_data)
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
    return "ERR"; // TODO: this
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
      std::cerr << tokenizer_filename
                << " is not an HFST archive" << std::endl
                << "Exception thrown:" << std::endl
                << e.what() << std::endl;
      return "ERR"; // TODO: this
    }
    if (first_header_attributes.count("name") != 0 || first_header_attributes["name"] == "TOP")
    {
      std::cout << "Treating as pmatch script..." << std::endl;
      hfst_ol::PmatchContainer container(instream);
      container.set_verbose(verbose);
      container.set_single_codepoint_tokenization(!settings.tokenize_multichar);
      if (process_input(container, std::cout) == EXIT_SUCCESS)
      {
        std::cout << "OUT:" << output.str() << std::endl;
        return output.str().c_str();
      }
      else
      {
        return "ERR"; // TODO: this
      }
    }
    else
    {
      std::cerr << "No TOP automaton found" << std::endl;
      return "ERR"; // TODO: this
    }
  }
  catch (HfstException &e)
  {
    std::cerr << "Exception thrown:" << std::endl
              << e.what() << std::endl;
    return "ERR"; // TODO: this
  }

  return "END OF TOKENIZE"; // TODO: this
}
