//! Word vectors and the 'Like()' and 'Unlike()' operations over them.

use super::*;

impl<B: AlgebraBackend + 'static> PmatchEvalContext<B> {
    // --- WORD_VECTORS (Vec<WordVector>) ---
    fn word_vectors_len(&self) -> usize {
        self.word_vectors.len()
    }
    fn word_vectors_clear(&mut self) {
        self.word_vectors.clear();
    }
    fn word_vectors_reserve(&mut self, n: usize) {
        self.word_vectors.reserve(n);
    }
    fn word_vectors_push(&mut self, wv: WordVector) {
        self.word_vectors.push(wv);
    }
    fn word_vectors_snapshot(&self) -> Vec<WordVector> {
        self.word_vectors.clone()
    }
    fn word_vectors_first_vector_len(&self) -> usize {
        self.word_vectors[0].vector.len()
    }
}

// Get the n best candidates in the original space using an insertion sort
// [spec:hfst:def:pmatch-utils.hfst.pmatch.get-top-n-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-top-n-fn]
pub fn get_top_n(
    n: usize,
    vecs: &[WordVector],
    comparison_point: &mut WordVector,
) -> Vec<(WordVector, WordVecFloat)> {
    let mut retval: Vec<(WordVector, WordVecFloat)> = Vec::new();
    for it in vecs.iter() {
        let cosdist: WordVecFloat = cosine_distance(it.clone(), comparison_point.clone());
        let mut i: usize = 0;
        while i <= retval.len() {
            if i == retval.len() {
                // We made it to the top
                retval.push((it.clone(), cosdist));
                break;
            } else {
                // Walking the list
                if cosdist >= retval[i].1 {
                    if i == 0 && retval.len() == n {
                        break;
                    }
                    retval.insert(i, (it.clone(), cosdist));
                    break;
                } else {
                    i += 1;
                    continue;
                }
            }
        }
        if retval.len() > n {
            retval.remove(0);
        }
    }
    retval
}
// Get the n best candidates in the transformed space using an insertion sort
// [spec:hfst:def:pmatch-utils.hfst.pmatch.get-top-n-transformed-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-top-n-transformed-fn]
pub fn get_top_n_transformed<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    n: usize,
    vecs: &[WordVector],
    plane_vec: Vec<WordVecFloat>,
    comparison_point: Vec<WordVecFloat>,
    translation_term: WordVecFloat,
    negative: bool,
) -> Vec<(WordVector, WordVecFloat)> {
    let mut retval: Vec<(WordVector, WordVecFloat)> = Vec::new();
    let plane_vec_square_sum: WordVecFloat = square_sum(plane_vec.clone());
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.norm-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.norm-fn]
    let comparison_point_norm: WordVecFloat = square_sum(comparison_point.clone()).sqrt();
    for it in vecs.iter() {
        let mut transformed_vec: WordVector = it.clone();

        /*
         * First, given a plane "plane_vec = translation term" and a point,
         * find the multiple of plane_vec which produces a vector going
         * from point to the nearest point in the plane.
         */

        let mut transformed_vec_scaler: WordVecFloat = (translation_term
            - dot_product(transformed_vec.vector.clone(), plane_vec.clone()))
            / plane_vec_square_sum;
        transformed_vec_scaler *= ctx.vector_similarity_projection_factor;
        if negative {
            transformed_vec.vector = pointwise_minus(
                transformed_vec.vector.clone(),
                pointwise_multiplication(transformed_vec_scaler, plane_vec.clone()),
            );
        } else {
            transformed_vec.vector = pointwise_plus(
                transformed_vec.vector.clone(),
                pointwise_multiplication(transformed_vec_scaler, plane_vec.clone()),
            );
        }
        transformed_vec.norm = square_sum(transformed_vec.vector.clone()).sqrt();
        let cosdist: WordVecFloat = 1.0
            - dot_product(transformed_vec.vector.clone(), comparison_point.clone())
                / (transformed_vec.norm * comparison_point_norm);
        let mut i: usize = 0;
        while i <= retval.len() {
            if i == retval.len() {
                // We made it to the top
                retval.push((transformed_vec.clone(), cosdist));
                break;
            } else {
                // Walking the list
                if cosdist >= retval[i].1 {
                    if i == 0 && retval.len() == n {
                        break;
                    }
                    retval.insert(i, (transformed_vec.clone(), cosdist));
                    break;
                } else {
                    i += 1;
                    continue;
                }
            }
        }
        if retval.len() > n {
            retval.remove(0);
        }
    }
    retval
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pointwise-minus-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pointwise-minus-fn]
pub fn pointwise_minus(l: Vec<WordVecFloat>, r: Vec<WordVecFloat>) -> Vec<WordVecFloat> {
    let mut ret: Vec<WordVecFloat> = vec![0 as WordVecFloat; l.len()];
    for i in 0..l.len() {
        ret[i] = l[i] - r[i];
    }
    ret
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pointwise-plus-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pointwise-plus-fn]
pub fn pointwise_plus(l: Vec<WordVecFloat>, r: Vec<WordVecFloat>) -> Vec<WordVecFloat> {
    let mut ret: Vec<WordVecFloat> = vec![0 as WordVecFloat; l.len()];
    for i in 0..l.len() {
        ret[i] = l[i] + r[i];
    }
    ret
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pointwise-multiplication-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pointwise-multiplication-fn]
pub fn pointwise_multiplication(scalar: WordVecFloat, r: Vec<WordVecFloat>) -> Vec<WordVecFloat> {
    let mut ret: Vec<WordVecFloat> = vec![0 as WordVecFloat; r.len()];
    for i in 0..r.len() {
        ret[i] = scalar * r[i];
    }
    ret
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.dot-product-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.dot-product-fn]
pub fn dot_product(l: Vec<WordVecFloat>, r: Vec<WordVecFloat>) -> WordVecFloat {
    let mut ret: WordVecFloat = 0 as WordVecFloat;
    for i in 0..l.len() {
        ret += l[i] * r[i];
    }
    ret
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.square-sum-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.square-sum-fn]
pub fn square_sum(v: Vec<WordVecFloat>) -> WordVecFloat {
    let mut ret: WordVecFloat = 0 as WordVecFloat;
    for x in v.iter() {
        ret += x * x;
    }
    ret
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.cosine-distance-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.cosine-distance-fn]
pub fn cosine_distance(left: WordVector, right: WordVector) -> WordVecFloat {
    // Sometimes very nearby vectors combined with rounding error will produce
    // a slightly negative distance, so make sure to return at least 0.0
    let retval: WordVecFloat =
        1.0 - dot_product(left.vector, right.vector) / (left.norm * right.norm);
    (0.0 as WordVecFloat).max(retval)
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.cosine-distance-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.cosine-distance-fn]
pub fn cosine_distance_vec(left: Vec<WordVecFloat>, right: Vec<WordVecFloat>) -> WordVecFloat {
    let retval: WordVecFloat = 1.0
        - dot_product(left.clone(), right.clone())
            / (square_sum(left).sqrt() * square_sum(right).sqrt());
    (0.0 as WordVecFloat).max(retval)
}
// the general case
// [spec:hfst:def:pmatch-utils.hfst.pmatch.compile-like-arc-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.compile-like-arc-fn]
pub fn compile_like_arc<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    word1: String,
    word2: String,
    nwords: u32,
    is_negative: bool,
) -> crate::error::Result<ObjRef<B>> {
    {
        let mut this_word1: WordVector = WordVector::default();
        let mut this_word2: WordVector = WordVector::default();
        {
            let wv_snapshot = ctx.word_vectors_snapshot();
            let mut it_iter = wv_snapshot.iter();
            loop {
                if !(this_word1.word.is_empty() || this_word2.word.is_empty()) {
                    break;
                }
                let it = match it_iter.next() {
                    Some(it) => it,
                    None => break,
                };
                if word1 == it.word {
                    this_word1 = it.clone();
                }
                if word2 == it.word {
                    this_word2 = it.clone();
                }
            }
        }
        if this_word1.word.is_empty() && this_word2.word.is_empty() {
            // got no matches
            let word1_o = Rc::new(PmatchString {
                name: String::new(),
                weight: 0.0,
                line_defined: 0,
                string: Symbol::from(word1),
                multichar: true,
                _marker: std::marker::PhantomData,
            });
            let word2_o = Rc::new(PmatchString {
                name: String::new(),
                weight: 0.0,
                line_defined: 0,
                string: Symbol::from(word2),
                multichar: true,
                _marker: std::marker::PhantomData,
            });
            pmatchwarning("no matches for arguments to Like() operation");
            let binop = Rc::new(PmatchBinaryOperation {
                name: String::new(),
                weight: 0.0,
                line_defined: 0,
                op: PmatchBinaryOp::Disjunct,
                left: as_obj(word1_o),
                right: as_obj(word2_o),
            });
            return Ok(as_obj(binop));
        }

        if this_word1.word.is_empty() || this_word2.word.is_empty() {
            // just one match
            pmatchwarning(
                "only one match for arguments to Like() operation, \
using nearest neighbours",
            );
            let mut this_word: WordVector = if this_word1.word.is_empty() {
                this_word2.clone()
            } else {
                this_word1.clone()
            };
            let top_n: Vec<(WordVector, WordVecFloat)> = get_top_n(
                nwords as usize,
                &ctx.word_vectors_snapshot(),
                &mut this_word,
            );
            let tok: HfstTokenizer = HfstTokenizer::new();
            let mut retval: HfstTransducer<B> = HfstTransducer::new();
            if ctx.verbose {
                debug!("Inserting into Like({}):", this_word.word);
            }

            for entry in top_n.iter() {
                if ctx.verbose {
                    debug!("  {}", entry.0.word);
                }
                let mut tmp: HfstTransducer<B> =
                    HfstTransducer::new_tokenized(&entry.0.word, &tok)?;
                if ctx.include_cosine_distances {
                    tmp.set_final_weights(entry.1, false)?;
                }
                retval.disjunct(&tmp, true)?;
            }
            let container = Rc::new(PmatchTransducerContainer {
                name: String::new(),
                weight: 0.0,
                line_defined: 0,
                t: retval,
            });
            return Ok(as_obj(container));
        }

        if ctx.variables_entry_or_default("vector-similarity-projection-factor") != "1.0" {
            ctx.vector_similarity_projection_factor =
                crate::string_manipulation::parse_float_prefix_str(
                    &ctx.variables_index("vector-similarity-projection-factor"),
                ) as WordVecFloat;
        }
        /*
         * When there are two vectors A and B, we compute the vector A - B that
         * goes from one to the other, and define a hyperplane orthogonal to that
         * vector that intersects the vector at the midpoint between the
         * two. We then add to all vectors a multiple of A - B to move them closer
         * to the plane, reducing the distance that is due to the difference
         * between A and B. (This is like projecting the space to the hyperplane
         * if we go all the way to the plane)
         *
         * The hyperplane is defined by the equation |B - A| = d, where d is a
         * translation term. |B - A| = 0 would be the set of vectors orthogonal to
         * |B - A|. We set d so that the distance from the hyperplane to A is
         * half of the norm of |B - A|.
         *
         */

        let B_minus_A: Vec<WordVecFloat> =
            pointwise_minus(this_word1.vector.clone(), this_word2.vector.clone());
        let hyperplane_translation_term: WordVecFloat =
            dot_product(B_minus_A.clone(), this_word1.vector.clone())
                - square_sum(B_minus_A.clone()) * 0.5;

        let comparison_point: Vec<WordVecFloat> = if is_negative {
            if ctx.verbose {
                debug!(
                    "Inserting into Unlike({}, {}):",
                    this_word1.word, this_word2.word
                );
            }
            let mut comparison_scaler: WordVecFloat = (hyperplane_translation_term
                - dot_product(this_word1.vector.clone(), B_minus_A.clone()))
                / square_sum(B_minus_A.clone());
            comparison_scaler *= ctx.vector_similarity_projection_factor;
            pointwise_minus(
                this_word1.vector.clone(),
                pointwise_multiplication(comparison_scaler, B_minus_A.clone()),
            )
        } else {
            if ctx.verbose {
                debug!(
                    "Inserting into Like({}, {}):",
                    this_word1.word, this_word2.word
                );
            }
            pointwise_plus(
                this_word2.vector.clone(),
                pointwise_multiplication(0.5 as WordVecFloat, B_minus_A.clone()),
            )
        };

        let top_n: Vec<(WordVector, WordVecFloat)> = get_top_n_transformed(
            ctx,
            nwords as usize,
            &ctx.word_vectors_snapshot(),
            B_minus_A.clone(),
            comparison_point,
            hyperplane_translation_term,
            is_negative,
        );
        let tok: HfstTokenizer = HfstTokenizer::new();
        let mut retval: HfstTransducer<B> = HfstTransducer::new();
        let mut i: usize = 0;
        while i < top_n.len() && i <= nwords as usize {
            if ctx.verbose {
                debug!("  {}", top_n[i].0.word);
            }
            let mut tmp: HfstTransducer<B> = HfstTransducer::new_tokenized(&top_n[i].0.word, &tok)?;
            if ctx.include_cosine_distances {
                tmp.set_final_weights(top_n[i].1, false)?;
            }
            retval.disjunct(&tmp, true)?;
            // if (include_cosine_distances) {
            //     for (size_t j = i + 1; j < word_vectors.size() && j <= nwords;
            //     ++j) {
            //         HfstTransducer tmp2(word_vectors[i].word + "_cos_" +
            //         word_vectors[j].word, tok, format);
            //         tmp2.set_final_weights(cosine_distance(projected_i,
            //                                                get_projected_vector(word_vectors[j].vector,
            //                                                B_minus_A,
            //                                                hyperplane_translation_term)));
            //         retval->disjunct(tmp2);
            //     }
            // }
            i += 1;
        }
        let container = Rc::new(PmatchTransducerContainer {
            name: String::new(),
            weight: 0.0,
            line_defined: 0,
            t: retval,
        });
        Ok(as_obj(container))
    }
}
// Single-word Like()
// [spec:hfst:def:pmatch-utils.hfst.pmatch.compile-like-arc-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.compile-like-arc-fn]
pub fn compile_like_arc_word<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    word: String,
    nwords: u32,
) -> crate::error::Result<ObjRef<B>> {
    {
        let mut this_word: WordVector = WordVector::default();
        for it in ctx.word_vectors_snapshot().iter() {
            if word == it.word {
                this_word = it.clone();
                break;
            }
        }
        if this_word.word.is_empty() {
            // got no matches
            let word_o = Rc::new(PmatchString {
                name: String::new(),
                weight: 0.0,
                line_defined: 0,
                string: Symbol::from(word),
                multichar: true,
                _marker: std::marker::PhantomData,
            });
            pmatchwarning("no matches for argument to Like() operation");
            return Ok(as_obj(word_o));
        }

        let top_n: Vec<(WordVector, WordVecFloat)> = get_top_n(
            nwords as usize,
            &ctx.word_vectors_snapshot(),
            &mut this_word,
        );

        let tok: HfstTokenizer = HfstTokenizer::new();
        let mut retval: HfstTransducer<B> = HfstTransducer::new();
        if ctx.verbose {
            debug!("Inserting into Like({}):", word);
        }
        for entry in top_n.iter() {
            if ctx.verbose {
                debug!("  {}", entry.0.word);
            }
            let mut tmp: HfstTransducer<B> = HfstTransducer::new_tokenized(&entry.0.word, &tok)?;
            if ctx.include_cosine_distances {
                tmp.set_final_weights(entry.1, false)?;
            }
            retval.disjunct(&tmp, true)?;
        }
        let container = Rc::new(PmatchTransducerContainer {
            name: String::new(),
            weight: 0.0,
            line_defined: 0,
            t: retval,
        });
        Ok(as_obj(container))
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.read-vec-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.read-vec-fn]
pub fn read_vec<B: AlgebraBackend + 'static>(ctx: &mut PmatchEvalContext<B>, filename: String) {
    use std::io::Read;
    let mut binary_format = false;
    if filename.len() >= 4 && filename.rfind(".bin") == Some(filename.len() - 4) {
        binary_format = true;
    }
    if ctx.word_vectors_len() != 0 {
        ctx.word_vectors_clear();
        warn!(
            "pmatch: vector model file {} overrides earlier one",
            filename
        );
    }
    let mut separator: u8 = b' ';
    let infile = match std::fs::File::open(&filename) {
        Ok(f) => f,
        Err(_) => {
            error!(
                "pmatch: could not open vector file {} for reading",
                filename
            );
            return;
        }
    };
    let mut infile = std::io::BufReader::new(infile);
    let mut all_bytes: Vec<u8> = Vec::new();
    if infile.read_to_end(&mut all_bytes).is_err() {
        error!(
            "pmatch: could not open vector file {} for reading",
            filename
        );
        return;
    }
    // Cursor over the raw file bytes, mirroring std::ifstream semantics.
    // Read the header line (up to '\n').
    let header_end = all_bytes
        .iter()
        .position(|&b| b == b'\n')
        .unwrap_or(all_bytes.len());
    let header_line: String = String::from_utf8_lossy(&all_bytes[..header_end]).into_owned();
    let mut cursor: usize = (header_end + 1).min(all_bytes.len());
    let lexicon_size: usize;
    let dimension: usize;
    {
        // ss >> lexicon_size; ss.ignore(1); ss >> dimension;
        let bytes = header_line.as_bytes();
        let mut p = 0;
        while p < bytes.len() && (bytes[p] as char).is_whitespace() {
            p += 1;
        }
        let ls_start = p;
        while p < bytes.len() && (bytes[p] as char).is_ascii_digit() {
            p += 1;
        }
        lexicon_size = header_line[ls_start..p].parse::<usize>().unwrap_or(0);
        if p < bytes.len() {
            p += 1; // ss.ignore(1)
        }
        while p < bytes.len() && (bytes[p] as char).is_whitespace() {
            p += 1;
        }
        let d_start = p;
        while p < bytes.len() && (bytes[p] as char).is_ascii_digit() {
            p += 1;
        }
        dimension = header_line[d_start..p].parse::<usize>().unwrap_or(0);
    }
    ctx.word_vectors_reserve(lexicon_size + 1);
    let mut words_read: usize = 0;
    if binary_format {
        let vector_data_size: usize = std::mem::size_of::<f32>() * dimension;
        while cursor < all_bytes.len() && words_read <= lexicon_size {
            // The actual number of vectors is 1 more than lexicon_size
            // due to <s>
            // std::getline(infile, line, separator)
            let word_end = all_bytes[cursor..]
                .iter()
                .position(|&b| b == separator)
                .map_or(all_bytes.len(), |i| cursor + i);
            let line = String::from_utf8_lossy(&all_bytes[cursor..word_end]).into_owned();
            cursor = (word_end + 1).min(all_bytes.len());
            // infile.read(&vector_data[0], vector_data_size)
            let read_end = std::cmp::min(cursor + vector_data_size, all_bytes.len());
            let vector_data: Vec<u8> = all_bytes[cursor..read_end].to_vec();
            cursor = read_end;
            // infile.ignore(1)
            if cursor < all_bytes.len() {
                cursor += 1;
            }
            // This will not compile is WordVectorFloat is not float,
            // in which case a conversion needs to happen, but
            // we can reasonably expect it to be a float for the
            // foreseeable future
            let mut comps: Vec<WordVecFloat> = Vec::new();
            let mut k = 0;
            while k + 4 <= vector_data.len() {
                let f = f32::from_ne_bytes([
                    vector_data[k],
                    vector_data[k + 1],
                    vector_data[k + 2],
                    vector_data[k + 3],
                ]);
                comps.push(f as WordVecFloat);
                k += 4;
            }
            let wv = WordVector {
                word: line,
                norm: square_sum(comps.clone()).sqrt(),
                vector: comps,
            };
            ctx.word_vectors_push(wv);
            words_read += 1;
        }
    } else {
        let text = &all_bytes[cursor..];
        // 'std::getline(infile, line)' consumes a trailing final newline
        // without yielding an extra empty line after it.
        let text = text.strip_suffix(b"\n").unwrap_or(text);
        for raw_line in text.split(|&b| b == b'\n') {
            if words_read > lexicon_size {
                break;
            }
            let line = String::from_utf8_lossy(raw_line).into_owned();
            if line.is_empty() {
                continue;
            }
            words_read += 1;
            let line_bytes = line.as_bytes();
            let mut pos = match line_bytes.iter().position(|&b| b == separator) {
                Some(p) => p,
                None => {
                    separator = b'\t';
                    match line_bytes.iter().position(|&b| b == separator) {
                        Some(p) => p,
                        None => {
                            warn!(
                                "pmatch: vector file {} doesn't appear to be tab- or \
space-separated\n  (reading line {})",
                                filename,
                                words_read + 1
                            );
                            break;
                        }
                    }
                }
            };
            let word: String = line[0..pos].to_string();
            let mut components: Vec<WordVecFloat> = Vec::new();
            // while (npos != (nextpos = line.find(separator, pos + 1)))
            loop {
                let nextpos = line_bytes[pos + 1..]
                    .iter()
                    .position(|&b| b == separator)
                    .map(|i| i + pos + 1);
                match nextpos {
                    Some(nextpos) => {
                        // line.substr(pos + 1, nextpos - pos)
                        let sub_end = std::cmp::min(pos + 1 + (nextpos - pos), line.len());
                        let sub = &line[pos + 1..sub_end];
                        let v =
                            crate::string_manipulation::parse_float_prefix_str(sub) as WordVecFloat;
                        components.push(v);
                        pos = nextpos;
                    }
                    None => break,
                }
            }
            // there can be one more from pos to the newline if there isn't a
            // separator at the end
            if line_bytes[line_bytes.len() - 1] != separator {
                let sub = &line[pos + 1..];
                let v = crate::string_manipulation::parse_float_prefix_str(sub) as WordVecFloat;
                components.push(v);
            }
            if ctx.word_vectors_len() != 0
                && ctx.word_vectors_first_vector_len() != components.len()
            {
                warn!(
                    "pmatch: vector file {} appears malformed\n  (reading line {})",
                    filename,
                    words_read + 1
                );
                continue;
            }
            let wv = WordVector {
                word,
                vector: components.clone(),
                norm: square_sum(components).sqrt(),
            };
            ctx.word_vectors_push(wv);
        }
    }
    if ctx.verbose {
        if ctx.word_vectors_len() == 0 {
            debug!("Tried to read word vector file, empty result");
        }
        debug!(
            "Read {} vectors of dimensionality {}",
            ctx.word_vectors_len(),
            ctx.word_vectors_first_vector_len()
        );
    }
}
