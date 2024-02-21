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

/*******************************************************************/
/*                                                                 */
/*     File: utf8.h                                                */
/*   Author: Helmut Schmid                                         */
/*  Purpose:                                                       */
/*  Created: Mon Sep  5 17:49:16 2005                              */
/* Modified: Mon Apr  7 08:26:39 2008 (schmid)                     */
/*                                                                 */
/*******************************************************************/

#ifndef _SFST_UTF8_H_
#define _SFST_UTF8_H_

/* @file HfstUtf8.h
   \brief Functions for handling utf-8 strings. */

namespace sfst_utf8 {

  unsigned int utf8toint( char *s );
  unsigned int utf8toint( char **s );
  char *int2utf8( unsigned int );

}
#endif
