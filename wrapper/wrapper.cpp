#include "wrapper.hpp"

// Global settings
static FILE *inputfile = stdin;

// TODO: tokenizer filename
static hfst::ImplementationType default_format = hfst::TROPICAL_OPENFST_TYPE;
hfst_ol_tokenize::TokenizeSettings settings;

void error(int status, int errnum, const char *fmt, ...)
{
  va_list ap;
  va_start(ap, fmt);
  vfprintf(stderr, fmt, ap);
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
  std::cout << "DELIM PRINT" << input_text << std::endl;
  if (!input_text.empty())
  {
    std::cout << "INPUT TEXT NOT EMPTY" << std::endl;
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

  std::cout << "INPUT DELIM" << std::endl;

  while ((len = hfst_getdelim(&line, &bufsize, '\0', inputfile)) > 0)
  {
    std::cout << "WHILE GOT DELIM" << std::endl;
    bool escaped = false; // Beginning of line is necessarily unescaped
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
        std::cout << "OPENING BRACKET" << std::endl;
        process_input_0delim_print(container, outstream, cur);
        cur << line[i];
        in_blank = true;
      }
      else if (do_superblank && in_blank && line[i] == ']')
      {
        std::cout << "CLOSING BRACKET" << std::endl;
        cur << line[i];
        if (i + 1 < len && line[i + 1] == '[')
        {
          // Join consecutive superblanks
          ++i;
          cur << line[i];
        }
        else
        {
          std::cout << "NONE OF THEM" << std::endl;
          in_blank = false;
          print_nonmatching_sequence(cur.str(), outstream, settings);
          cur.clear();
          cur.str(string());
        }
      }
      else if (!in_blank && line[i] == '\n')
      {
        std::cout << "END OF LINE" << std::endl;
        cur << line[i];
        process_input_0delim_print(container, outstream, cur);
      }
      else if (line[i] == '\0')
      {
        std::cout << "NULL CHARACTER" << std::endl;
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
        std::cout << "ELSE" << std::endl;
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
    std::cout << "IN BLANK" << std::endl;
    print_nonmatching_sequence(cur.str(), outstream, settings);
  }
  else
  {
    std::cout << "NOT IN BLANK" << std::endl;
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
}

extern "C" const char *hfst_tokenize(const uint8_t *input, size_t input_size, const uint8_t* tokenizer, size_t tokenizer_size)
{
  std::ostringstream output;
  std::string input_str( input, input+input_size );
  std::string tokenizer_filename( tokenizer, tokenizer+tokenizer_size );

  // Settings to output CG format used in Giella infrastructure
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

  std::cout << "Reading from" << tokenizer_filename << std::endl;
  std::ifstream instream(tokenizer_filename, std::ifstream::binary);
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
    catch (TransducerHeaderException &err)
    {
      std::cerr << tokenizer_filename
                << " is not an HFST archive" << std::endl
                << "Exception thrown:" << std::endl
                << err.what() << std::endl;
      return "ERR"; // TODO: this
    }

    if (first_header_attributes.count("name") == 0 || first_header_attributes["name"] != "TOP")
    {
      std::cerr << "No TOP automaton found" << std::endl;
      return "ERR"; // TODO: this
    }

    hfst_ol::PmatchContainer container(instream);
    container.set_verbose(false);
    container.set_single_codepoint_tokenization(!settings.tokenize_multichar);

    if (process_input(container, output) != EXIT_SUCCESS)
    {
      return "ERR"; // TODO: this
    }

    std::cout << "OUT:" << output.str() << std::endl;
    return output.str().c_str();
  }
  catch (HfstException &err)
  {
    std::cerr << "Exception thrown:" << std::endl
              << err.what() << std::endl;
    return "ERR"; // TODO: this
  }

  return "END OF TOKENIZE"; // TODO: this
}
