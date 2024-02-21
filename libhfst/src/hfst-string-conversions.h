// This library is free software; you can redistribute it and/or
// modify it under the terms of the GNU Lesser General Public
// License as published by the Free Software Foundation; either
// version 3 of the License, or (at your option) any later version.
// See the file COPYING included with this distribution for more
// information.

#ifdef WINDOWS

#include <string>
#include <cstdio>

namespace hfst
{
  /* Convert utf-8 string \a wstr into an ordinary string. */
  //std::string wide_string_to_string(const std::wstring & wstr);
  /* Get a line from console input and store it into \a str.
         \a buffer_size defines maximum of input length.
         Return whether the read operation was succesful. */
  bool get_line_from_console(std::string & str, size_t buffer_size, bool keep_newline = false);

  void set_console_cp_to_utf8();

  /* Wrapper around fprintf that prints to console output if stream is stdout or stderr.
         On linux and mac, calls always fprintf directly. */
  int hfst_fprintf_console(FILE * stream, const char * format, ...);
}

#endif // WINDOWS
