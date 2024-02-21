// This library is free software; you can redistribute it and/or
// modify it under the terms of the GNU Lesser General Public
// License as published by the Free Software Foundation; either
// version 3 of the License, or (at your option) any later version.
// See the file COPYING included with this distribution for more
// information.

/*
   HFST has a written agreement with the author of SFST, Helmut Schmid,
   that this file, though derived from SFST which is GPLv2+,
   may be distributed under the LGPLv3+ license as part of HFST.
*/

#ifndef _SFST_BASIC_H_
#define _SFST_BASIC_H_

#include <stdio.h>

/* @file SfstBasic.h
   \brief Basic functions for string handling. */

namespace sfst_basic {

  extern bool Switch_Bytes;

  char* fst_strdup(const char* pString);
  int read_string( char *buffer, int size, FILE *file );
  size_t read_num( void *p, size_t size, FILE *file );

}
#endif
