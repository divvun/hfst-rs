//! @file hfst-check-alpha.cc
//!
//! @brief compare alphabets within and between automata
//!
//! @author HFST Team

//  This program is free software: you can redistribute it and/or modify
//  it under the terms of the GNU General Public License as published by
//  the Free Software Foundation, version 3 of the License.
//
//  This program is distributed in the hope that it will be useful,
//  but WITHOUT ANY WARRANTY; without even the implied warranty of
//  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//  GNU General Public License for more details.
//
//  You should have received a copy of the GNU General Public License
//  along with this program.  If not, see <http://www.gnu.org/licenses/>.

#ifdef HAVE_CONFIG_H
#include <config.h>
#endif

#ifdef WINDOWS
#include <io.h>
#endif

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <getopt.h>
#include <memory>

#include "HfstInputStream.h"
#include "HfstOutputStream.h"
#include "HfstTransducer.h"
#include "implementations/HfstBasicTransducer.h"

using std::map;
using std::string;

using hfst::HfstInputStream;
using hfst::HfstTransducer;
using hfst::StringSet;
using hfst::implementations::HfstBasicTransducer;
using hfst::implementations::HfstState;
using hfst::implementations::HfstTransitionGraph;

#include "hfst-commandline.h"
#include "hfst-program-options.h"

#include "inc/globals-binary.h"
#include "inc/globals-common.h"

// add tools-specific variables here

void
print_usage()
{
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    fprintf(message_out,
            "Usage: %s [OPTIONS...] [INFILEs]\n"
            "Compare the compatibility of alphabets between INFILEs\n"
            "\n",
            program_name);

    print_common_program_options(message_out);
    print_common_binary_program_options(message_out);
    // fprintf(message_out, (tool-specific options and short descriptions)
    fprintf(message_out, "Check alpha options:\n");
    fprintf(message_out, "\n");
    print_common_binary_program_parameter_instructions(message_out);
    fprintf(message_out, "\n");
    print_report_bugs();
    fprintf(message_out, "\n");
    print_more_info();
}

void
fprint_stringset(FILE *outfile, const StringSet &strings)
{
    bool first = true;
    for (auto s : strings)
    {
        if (!first)
        {
            fprintf(outfile, ", ");
        }
        fprintf(outfile, "%s", s.c_str());
        first = false;
    }
}

int
parse_options(int argc, char **argv)
{
    extend_options_getenv(&argc, &argv);
    // use of this function requires options are settable on global scope
    while (true)
    {
        static const struct option long_options[]
            = { HFST_GETOPT_COMMON_LONG,
                HFST_GETOPT_BINARY_LONG,
                // add tool-specific options here
                { 0, 0, 0, 0 } };
        int option_index = 0;
        // add tool-specific options here
        int c = getopt_long(
            argc, argv, HFST_GETOPT_COMMON_SHORT HFST_GETOPT_BINARY_SHORT "",
            long_options, &option_index);
        if (-1 == c)
        {
            break;
        }

        switch (c)
        {
#include "inc/getopt-cases-binary.h"
#include "inc/getopt-cases-common.h"
            // add tool-specific cases here
#include "inc/getopt-cases-error.h"
        }
    }

#include "inc/check-params-binary.h"
#include "inc/check-params-common.h"
    return EXIT_CONTINUE;
}

int
process_stream(HfstInputStream &firststream, HfstInputStream &secondstream)
{
    bool continueReading = firststream.is_good() && secondstream.is_good();
    size_t transducer_n = 0;
    int mismatch = EXIT_SUCCESS;
    while (continueReading)
    {
        transducer_n++;

        if (transducer_n < 2)
        {
            verbose_printf("Checking alphas...\n");
        }
        else
        {
            verbose_printf("Checking alphas... " SIZE_T_SPECIFIER "\n",
                           transducer_n);
        }
        // read first alphas
        HfstTransducer first{ firststream };
        HfstBasicTransducer mutt{ first };
        StringSet firstTransducerAlphabet;
        bool transducerKnowsAlphabet = false;
        try
        {
            firstTransducerAlphabet = first.get_alphabet();
            transducerKnowsAlphabet = true;
        }
        catch (FunctionNotImplementedException)
        {
            transducerKnowsAlphabet = false;
        }
        StringSet firstFoundAlphabet;
        // iterate states in random order
        for (HfstBasicTransducer::const_iterator it = mutt.begin();
             it != mutt.end(); it++)
        {
            for (hfst::implementations::HfstBasicTransitions::const_iterator
                     tr_it
                 = it->begin();
                 tr_it != it->end(); tr_it++)
            {
                firstFoundAlphabet.insert(tr_it->get_input_symbol());
                firstFoundAlphabet.insert(tr_it->get_output_symbol());
            }
        }
        // read second alphas
        HfstTransducer second{ secondstream };
        HfstBasicTransducer secondmutt{ second };
        StringSet secondTransducerAlphabet;
        transducerKnowsAlphabet = false;
        try
        {
            secondTransducerAlphabet = second.get_alphabet();
            transducerKnowsAlphabet = true;
        }
        catch (FunctionNotImplementedException)
        {
            transducerKnowsAlphabet = false;
        }
        StringSet secondFoundAlphabet;
        // iterate states in random order
        for (HfstBasicTransducer::const_iterator it = secondmutt.begin();
             it != secondmutt.end(); it++)
        {
            for (hfst::implementations::HfstBasicTransitions::const_iterator
                     tr_it
                 = it->begin();
                 tr_it != it->end(); tr_it++)
            {
                secondFoundAlphabet.insert(tr_it->get_input_symbol());
                secondFoundAlphabet.insert(tr_it->get_output_symbol());
            }
        }
        // match
        fprintf(outfile, "Actual alphabet differences:\n");
        StringSet firstMinusSecond;
        std::set_difference(
            firstFoundAlphabet.begin(), firstFoundAlphabet.end(),
            secondFoundAlphabet.begin(), secondFoundAlphabet.end(),
            std::inserter(firstMinusSecond, firstMinusSecond.end()));
        if (!firstMinusSecond.empty())
        {
            mismatch = EXIT_FAILURE;
            fprintf(outfile, "In first %s but not in second %s:",
                    first.get_name().c_str(), second.get_name().c_str());
            fprint_stringset(outfile, firstMinusSecond);
        }
        else
        {
            fprintf(outfile, "First %s alpha is superset of second %s.",
                    first.get_name().c_str(), second.get_name().c_str());
        }
        fprintf(outfile, "\n");
        StringSet secondMinusFirst;
        std::set_difference(
            secondFoundAlphabet.begin(), secondFoundAlphabet.end(),
            firstFoundAlphabet.begin(), firstFoundAlphabet.end(),
            std::inserter(secondMinusFirst, secondMinusFirst.end()));
        if (!secondMinusFirst.empty())
        {
            mismatch = EXIT_FAILURE;
            fprintf(outfile, "In second %s but not in first %s:",
                    second.get_name().c_str(), second.get_name().c_str());
            fprint_stringset(outfile, secondMinusFirst);
        }
        else
        {
            fprintf(outfile, "Second %s alpha is superset of second %s.",
                    second.get_name().c_str(), second.get_name().c_str());
        }
        fprintf(outfile, "\n");
        if (verbose)
        {
            fprintf(outfile, "%s alphabet:", first.get_name().c_str());
            fprint_stringset(outfile, firstFoundAlphabet);
            fprintf(outfile, "\n");
            fprintf(outfile, "%s alphabet:", second.get_name().c_str());
            fprint_stringset(outfile, secondFoundAlphabet);
            fprintf(outfile, "\n");
        }
        if (transducerKnowsAlphabet)
        {
            fprintf(outfile, "sigma set difference:\n");
            StringSet firstMinusSecond;
            std::set_difference(
                firstTransducerAlphabet.begin(), firstTransducerAlphabet.end(),
                secondTransducerAlphabet.begin(),
                secondTransducerAlphabet.end(),
                std::inserter(firstMinusSecond, firstMinusSecond.end()));
            StringSet secondMinusFirst;
            std::set_difference(
                secondTransducerAlphabet.begin(),
                secondTransducerAlphabet.end(),
                firstTransducerAlphabet.begin(), firstTransducerAlphabet.end(),
                std::inserter(secondMinusFirst, secondMinusFirst.end()));
            if (!firstMinusSecond.empty())
            {
                mismatch = EXIT_FAILURE;
                fprintf(outfile, "First %s has but second %s does not: ",
                        first.get_name().c_str(), second.get_name().c_str());
                fprint_stringset(outfile, firstMinusSecond);
            }
            else
            {
                fprintf(outfile, "First %s alpha is superset of second %s.",
                        first.get_name().c_str(), second.get_name().c_str());
            }
            fprintf(outfile, "\n");
            if (!secondMinusFirst.empty())
            {
                mismatch = EXIT_FAILURE;
                fprintf(outfile, "Second %s has but first %s does not: ",
                        second.get_name().c_str(), first.get_name().c_str());
                fprint_stringset(outfile, secondMinusFirst);
            }
            else
            {
                fprintf(outfile, "Second %s alpha is superset of first %s.",
                        second.get_name().c_str(), first.get_name().c_str());
            }
            fprintf(outfile, "\n");
            if (verbose)
            {
                fprintf(outfile, "First (%s):", first.get_name().c_str());
                fprint_stringset(outfile, firstTransducerAlphabet);
                fprintf(outfile, "\n");
                fprintf(outfile, "Second (%s):", second.get_name().c_str());
                fprint_stringset(outfile, secondTransducerAlphabet);
                fprintf(outfile, "\n");
            }
        }
        else
        {
            fprintf(outfile,
                    "No internal alphabets to compare in this format\n");
        } // FSTs know their alphas
        continueReading = firststream.is_good() && secondstream.is_good();
    }

    fprintf(outfile, "\nRead " SIZE_T_SPECIFIER " transducers in total.\n",
            transducer_n);
    return mismatch;
}

int
main(int argc, char **argv)
{
#ifdef WINDOWS
    _setmode(0, _O_BINARY);
#endif

    hfst_set_program_name(argv[0], "0.1", "HfstALphaFix");
    int retval = parse_options(argc, argv);
    if (retval != EXIT_CONTINUE)
    {
        return retval;
    }
    // close buffers, we use streams
    if (firstfile != stdin)
    {
        fclose(firstfile);
    }
    if (secondfile != stdin)
    {
        fclose(secondfile);
    }
    verbose_printf("Reading from %s and %s, writing to %s\n", firstfilename,
                   secondfilename, outfilename);
    // here starts the buffer handling part
    std::unique_ptr<HfstInputStream> firststream;
    std::unique_ptr<HfstInputStream> secondstream;
    try
    {
        firststream.reset((firstfile != stdin)
                              ? new HfstInputStream(firstfilename)
                              : new HfstInputStream());
    } // NotTransducerStreamException
    catch (const HfstException e)
    {
        error(EXIT_FAILURE, 0, "%s is not a valid transducer file",
              firstfilename);
        return EXIT_FAILURE;
    }
    try
    {
        secondstream.reset((secondfile != stdin)
                               ? new HfstInputStream(secondfilename)
                               : new HfstInputStream());
    } // NotTransducerStreamException
    catch (const HfstException e)
    {
        error(EXIT_FAILURE, 0, "%s is not a valid transducer file",
              secondfilename);
        return EXIT_FAILURE;
    }

    retval = process_stream(*firststream, *secondstream);

    if (outfile != stdout)
    {
        fclose(outfile);
    }
    free(firstfilename);
    free(secondfilename);
    free(outfilename);
    return EXIT_SUCCESS;
}
