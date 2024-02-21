// This library is free software; you can redistribute it and/or
// modify it under the terms of the GNU Lesser General Public
// License as published by the Free Software Foundation; either
// version 3 of the License, or (at your option) any later version.
// See the file COPYING included with this distribution for more
// information.

// Windows-specific globals
#ifdef WINDOWS
#ifndef _WINDOWS_STREAM_BINARY_MODE
#define _WINDOWS_STREAM_BINARY_MODE
#include <fcntl.h>
// Make sure that all standard streams are in binary mode.
// Line feed and carriage return characters are not handled correctly
// in text mode.
int _CRT_fmode = _O_BINARY;
#endif // _WINDOWS_STREAM_BINARY_MODE
#endif // WINDOWS
