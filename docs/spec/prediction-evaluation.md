# Measuring prediction

A language's prediction settings are chosen by measuring them on its own
text: the model's order and backoff, the grammar's boundaries, how many
suggestions show and what a keyboard face holds. The harness these rules
describe does the measuring. One measure is used for every language and every
system compared, so that results can be set side by side.

## The measure

> [spec:hfst:def:prediction-evaluation.keystrokes]
> Keystrokes saved is one minus the number of taps divided by the number of
> characters, over held-out text. The characters are those of the text's
> words as written, with spaces and punctuation left out. The taps are a
> simulated user's: one for each letter typed, each suggestion picked, each
> face key pressed and each switch of face, and two for each long press.
>
> The simulated user knows each word's analysis. At each step it picks, from
> the N suggestions shown and the keys of the face shown, the word's next unit
> or the complete word, when one of them is offered. Otherwise it types the
> word's next letter, which keeps only the suggestions whose preview
> (`prediction.preview`) starts with the letters typed since the last unit.
> A word the walk cannot reach is typed in full. N, the faces and which
> units sit behind a long press are settings of the run.

> [spec:hfst:req:prediction-evaluation.held-out]
> Text that is scored MUST NOT be part of the text the scored model was built
> from. The harness MUST split a corpus into k parts, build k models that
> each leave one part out, and score each model on the part it left out. k is
> a setting, 10 when not given.

## Other systems

> [spec:hfst:req:prediction-evaluation.external]
> The harness MUST score suggestions recorded from another system by the same
> measure. A recording gives, for each word of the text and each of its
> prefixes, the suggestions that system shows for that prefix, in order. The
> simulated user picks the longest suggestion shown that the word starts
> with, or else types the next letter. This is how a speller that completes
> letters to the next morpheme boundary, such as lang-crk's, is compared with
> unit prediction.

## The report

> [spec:hfst:req:prediction-evaluation.report]
> A run MUST report, for each variant scored, its keystrokes saved, how often
> the word's next unit was the first suggestion, and how often it was among
> the N shown, each overall and by the number of units in the word. Variants
> scored on the same text MUST be reported side by side.
