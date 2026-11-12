"""Build analysed North Sámi text for hfst predict-eval from corpus-sme.

Two steps, each a subcommand:

  extract  Choose documents from the converted XML of the open corpus,
           genre by genre, keep their North Sámi paragraphs and cut them
           into tokens, one per line. Also writes a manifest of the
           documents used.
  analyse  Analyse each token with hfst lookup and write the tokens in
           hfst-lookup format, as hfst predict-model and predict-eval read
           them.

Run with python3 -I. Standard library only.
"""

import argparse
import random
import re
import subprocess
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

XML_LANG = "{http://www.w3.org/XML/1998/namespace}lang"

# A word is a run of letters and digits, which may hold a hyphen, an
# apostrophe or a colon between two of them (2010-logu, NRK:s) and may end
# with a hyphen (álbmot- in álbmot- ja servodat). Anything else that is not
# a space is a token of one character.
TOKEN = re.compile(r"[^\W_](?:[^\W_]|[-'’ʼ:](?=[^\W_]))*-?|\S")


def is_word(token):
    return any(c.isalnum() for c in token)


def text_of(element):
    """The North Sámi text of an element: its own text and its children's,
    leaving out children marked as another language but keeping the text
    after them."""
    parts = [element.text or ""]
    for child in element:
        if child.get(XML_LANG, "sme") == "sme":
            parts.append(text_of(child))
        parts.append(child.tail or "")
    return "".join(parts)


def paragraphs(path):
    """The running-text paragraphs of a converted document: p elements of
    the body with no type, or type text, not marked as another language.
    Titles, list items and table cells are left out."""
    root = ET.parse(path).getroot()
    if root.get(XML_LANG) != "sme":
        return []
    body = root.find("body")
    if body is None:
        return []
    kept = []
    for p in body.iter("p"):
        if p.get("type", "text") != "text" or p.get(XML_LANG, "sme") != "sme":
            continue
        text = " ".join(text_of(p).split())
        if text:
            kept.append(text)
    return kept


def choose_documents(corpus, genre, words, max_doc_words, rng):
    """Documents of one genre in a seeded random order, each cut to at most
    max_doc_words words at a paragraph's end, until the genre has words
    words. Returns (path, tokens) pairs."""
    paths = sorted((corpus / genre).rglob("*.xml"))
    rng.shuffle(paths)
    chosen = []
    total = 0
    for path in paths:
        if total >= words:
            break
        tokens = []
        doc_words = 0
        for paragraph in paragraphs(path):
            if doc_words >= max_doc_words or total + doc_words >= words:
                break
            found = TOKEN.findall(paragraph)
            tokens.extend(found)
            doc_words += sum(1 for token in found if is_word(token))
        if doc_words:
            chosen.append((path, tokens))
            total += doc_words
    return chosen


def extract(args):
    corpus = Path(args.corpus)
    rng = random.Random(args.seed)
    documents = []
    for genre in args.genres.split(","):
        for path, tokens in choose_documents(
            corpus, genre, args.words_per_genre, args.max_doc_words, rng
        ):
            documents.append((genre, path.relative_to(corpus), tokens))
    # Interleave the genres, so each of the harness's contiguous folds holds
    # some of every genre.
    rng.shuffle(documents)
    with open(args.tokens, "w", encoding="utf-8") as out, open(
        args.manifest, "w", encoding="utf-8"
    ) as manifest:
        manifest.write("genre\twords\ttokens\tfile\n")
        for genre, path, tokens in documents:
            words = sum(1 for token in tokens if is_word(token))
            manifest.write(f"{genre}\t{words}\t{len(tokens)}\t{path}\n")
            for token in tokens:
                out.write(token + "\n")
    by_genre = {}
    for genre, _, tokens in documents:
        by_genre[genre] = by_genre.get(genre, 0) + sum(1 for t in tokens if is_word(t))
    for genre, words in by_genre.items():
        print(f"{genre}\t{words} words", file=sys.stderr)
    print(f"total\t{sum(by_genre.values())} words", file=sys.stderr)


def lookup(hfst, analyser, forms):
    """hfst lookup's readings for each form, in its order. A reading it
    gives twice, by two paths through the analyser, is kept once, at its
    first weight."""
    run = subprocess.run(
        [hfst, "lookup", "-q", "--pipe-mode=input", analyser],
        input="".join(form + "\n" for form in forms),
        capture_output=True,
        text=True,
        check=True,
    )
    blocks = run.stdout.split("\n\n")
    readings = {}
    for form, block in zip(forms, blocks):
        found = []
        for line in block.strip("\n").split("\n"):
            fields = line.split("\t")
            if len(fields) != 3 or fields[0] != form:
                raise SystemExit(f"unexpected hfst lookup line for {form!r}: {line!r}")
            if fields[1] != form + "+?" and all(fields[1] != r for r, _ in found):
                found.append((fields[1], fields[2]))
        readings[form] = found
    missing = [form for form in forms if form not in readings]
    if missing:
        raise SystemExit(f"hfst lookup gave no block for {missing[:5]}")
    return readings


def analyse(args):
    with open(args.tokens, encoding="utf-8") as f:
        tokens = [line.rstrip("\n") for line in f if line.strip()]
    forms = sorted(set(tokens))
    readings = lookup(args.hfst, args.analyser, forms)
    # A token in capitals that the analyser does not know is analysed in
    # small letters; capitals are the keyboard's shift state.
    lowered = sorted(
        {form.lower() for form in forms if not readings[form] and form.lower() != form}
    )
    lower_readings = lookup(args.hfst, args.analyser, lowered)
    unknown = 0
    from_lower = 0
    with open(args.out, "w", encoding="utf-8") as out:
        for token in tokens:
            found = readings[token]
            if not found and token.lower() in lower_readings:
                found = lower_readings[token.lower()]
                from_lower += bool(found)
            if not found:
                unknown += 1
                out.write(f"{token}\t{token}+?\tinf\n\n")
                continue
            for reading, weight in found:
                out.write(f"{token}\t{reading}\t{weight}\n")
            out.write("\n")
    print(
        f"{len(tokens)} tokens, {len(forms)} forms, {unknown} tokens unknown, "
        f"{from_lower} analysed in small letters",
        file=sys.stderr,
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sub = parser.add_subparsers(dest="command", required=True)
    e = sub.add_parser("extract", help="choose documents and cut them into tokens")
    e.add_argument("--corpus", required=True, help="corpus-sme/converted")
    e.add_argument("--genres", required=True, help="genre directories, by commas")
    e.add_argument("--words-per-genre", type=int, default=6000)
    e.add_argument("--max-doc-words", type=int, default=1500)
    e.add_argument("--seed", type=int, default=1)
    e.add_argument("--tokens", required=True, help="tokens out, one per line")
    e.add_argument("--manifest", required=True, help="documents used, TSV")
    a = sub.add_parser("analyse", help="analyse tokens into hfst-lookup format")
    a.add_argument("--hfst", required=True, help="the hfst binary")
    a.add_argument("--analyser", required=True, help="analyser-gt-norm.hfstol")
    a.add_argument("--tokens", required=True)
    a.add_argument("--out", required=True)
    args = parser.parse_args()
    if args.command == "extract":
        extract(args)
    else:
        analyse(args)


if __name__ == "__main__":
    main()
