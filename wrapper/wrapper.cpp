#include "wrapper.hpp"

static hfst_ol_tokenize::TokenizeSettings init_settings() {
  hfst_ol_tokenize::TokenizeSettings settings;

  settings.output_format = hfst_ol_tokenize::giellacg;
  settings.print_weights = true;
  settings.print_all = true;
  settings.dedupe = true;
  settings.hack_uncompose = true;
  settings.verbose = false;
  if (settings.max_weight_classes == std::numeric_limits<int>::max()) {
    settings.max_weight_classes = 2;
  }
  return settings;
}

hfst_ol_tokenize::TokenizeSettings settings = init_settings();

class membuf : public std::basic_streambuf<char> {
public:
  membuf(const uint8_t *p, size_t l) {
    setg((char *)p, (char *)p, (char *)p + l);
  }
};

class memstream : public std::istream {
public:
  memstream(const uint8_t *p, size_t l)
      : std::istream(&_buffer), _buffer(p, l) {
    rdbuf(&_buffer);
  }

private:
  membuf _buffer;
};

inline void process_input_0delim_print(hfst_ol::PmatchContainer &container,
                                       std::ostream &outstream,
                                       std::ostringstream &cur) {
  const std::string &input_text{cur.str()};
  if (!input_text.empty()) {
    match_and_print(container, outstream, input_text, settings);
  }
  cur.clear();
  cur.str(string());
}

template <bool do_superblank>
int process_input_0delim(hfst_ol::PmatchContainer &container,
                         std::istream &infile, std::ostream &outstream) {
  bool in_blank = false;
  std::ostringstream cur;

  std::string line;
  // char c;
  while (!infile.eof()) {
    getline(infile, line, '\0');

    bool escaped = false; // Beginning of line is necessarily unescaped
    for (unsigned long i = 0; i < line.length(); ++i) {
      if (escaped) {
        cur << line[i];
        escaped = false;
        continue;
      } else if (do_superblank && !in_blank && line[i] == '[') {
        process_input_0delim_print(container, outstream, cur);
        cur << line[i];
        in_blank = true;
      } else if (do_superblank && in_blank && line[i] == ']') {
        cur << line[i];
        if (i + 1 < line.length() && line[i + 1] == '[') {
          // Join consecutive superblanks
          ++i;
          cur << line[i];
        } else {
          in_blank = false;
          print_nonmatching_sequence(cur.str(), outstream, settings);
          cur.clear();
          cur.str(string());
        }
      } else if (!in_blank && line[i] == '\n') {
        cur << line[i];
        process_input_0delim_print(container, outstream, cur);
      } else if (line[i] == '\0') {
        process_input_0delim_print(container, outstream, cur);
        outstream << "<STREAMCMD:FLUSH>"
                  << std::endl; // CG format uses this instead of \0
        outstream.flush();
        if (outstream.bad()) {
          std::cerr << "hfst-tokenize: Could not flush file" << std::endl;
        }
      } else {
        cur << line[i];
      }
      escaped = (line[i] == '\\');
    }
  }

  if (in_blank) {
    print_nonmatching_sequence(cur.str(), outstream, settings);
  } else {
    process_input_0delim_print(container, outstream, cur);
  }

  return EXIT_SUCCESS;
}

int process_input(hfst_ol::PmatchContainer &container, std::istream &infile,
                  std::ostream &outstream) {
  outstream << std::fixed << std::setprecision(10);

  // Processing giellacg without superblanks
  return process_input_0delim<false>(container, infile, outstream);
}

extern "C" const hfst_ol::PmatchContainer *
hfst_make_tokenizer(const char *tokenizer_bytes, size_t tokenizer_size) {
  // Settings to output CG format used in Giella infrastructure
  std::stringstream tokenizer;
  tokenizer.write(tokenizer_bytes, tokenizer_size);
  tokenizer.seekg(0);

  try {
    std::map<std::string, std::string> first_header_attributes;
    try {
      first_header_attributes =
          hfst_ol::PmatchContainer::parse_hfst3_header(tokenizer);
      tokenizer.seekg(0);
      tokenizer.clear();
    } catch (TransducerHeaderException &err) {
      std::cerr << "Not an HFST archive" << std::endl
                << "Exception thrown:" << std::endl
                << err.what() << std::endl;
      return nullptr;
    }

    if (first_header_attributes.count("name") == 0 ||
        first_header_attributes["name"] != "TOP") {
      std::cerr << "No TOP automaton found" << std::endl;
      return nullptr;
    }

    auto container = new hfst_ol::PmatchContainer(tokenizer);
    container->set_verbose(false);
    container->set_single_codepoint_tokenization(!settings.tokenize_multichar);

    return container;
  } catch (HfstException &err) {
    std::cerr << "Exception thrown:" << std::endl << err.what() << std::endl;
    return nullptr;
  }
}

extern "C" const char *hfst_tokenize(hfst_ol::PmatchContainer &tokenizer,
                                     const uint8_t *input, size_t input_size) {
  std::ostringstream output;

  std::string input_str(input, input + input_size);
  memstream text(input, input_size);

  if (process_input(tokenizer, text, output) != EXIT_SUCCESS) {
    return nullptr;
  }

  char *c_str = strdup(output.str().c_str());
  return c_str;
}

extern "C" void hfst_tokenizer_free(hfst_ol::PmatchContainer *ptr) {
  delete ptr;
}

extern "C" void hfst_free(void *ptr) { free(ptr); }

extern "C" void hfst_transducer_free(hfst::HfstTransducer *ptr) { delete ptr; }

extern "C" void hfst_transducer_lookup_tags(
    hfst::HfstTransducer *analyzer, bool is_diacritic, const char *input,
    size_t input_size, double time_cutoff, void *tags,
    void (*callback)(void *tags, const char *, size_t)) {
  
  std::cerr << "hfst_transducer_lookup_tags" << std::endl;

  std::string input_str(input, input + input_size);
  hfst::HfstOneLevelPaths *results =
      analyzer->lookup_fd(input_str, -1, time_cutoff);

  std::cerr << "results: " << results->size() << std::endl;

  for (auto result : *results) {
    std::cerr << "result: " << result.first << std::endl;
    auto string_builder = std::stringstream();
    for (auto ss : result.second) {
      std::cerr << "ss: " << ss << std::endl;

      if (is_diacritic ? hfst::FdOperation::is_diacritic(ss)
                       : !hfst::FdOperation::is_diacritic(ss)) {
        string_builder << ss;
      }
    }
    auto s = string_builder.str();
    (callback)(tags, s.c_str(), s.length());
  }
}

extern "C" const hfst::HfstTransducer *
hfst_transducer_new(const uint8_t *analyzer_bytes, size_t analyzer_size) {
  memstream analyzer_data(analyzer_bytes, analyzer_size);
  hfst::HfstInputStream *in;
  try {
    in = new hfst::HfstInputStream(analyzer_data);
  } catch (StreamNotReadableException &e) {
    std::cerr << "ERROR: File does not exist." << std::endl;
    return nullptr;
  } catch (HfstException &e) {
    std::cerr << "ERROR: HfstException: " << e.what() << std::endl;
    return nullptr;
  }

  hfst::HfstTransducer *t = nullptr;

  while (!in->is_eof()) {
    if (in->is_bad()) {
      std::cerr << "ERROR: Stream cannot be read." << std::endl;
      return nullptr;
    }

    t = new hfst::HfstTransducer(*in);

    if (!in->is_eof()) {
      std::cerr << "WARNING: >1 transducers in stream! Only using the first."
                << std::endl;
    }

    break;
  }

  in->close();
  delete in;

  if (t == nullptr) {
    std::cerr << "WARNING: Could not read any transducers!" << std::endl;
  }

  return t;
}
