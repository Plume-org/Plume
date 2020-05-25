#[cfg(feature = "search-lindera")]
use lindera_tantivy::tokenizer::LinderaTokenizer;
use std::str::CharIndices;
use tantivy::tokenizer::*;

#[derive(Clone, Copy)]
pub enum TokenizerKind {
    Simple,
    Ngram,
    Whitespace,
    #[cfg(feature = "search-lindera")]
    Lindera,
}

impl From<TokenizerKind> for TextAnalyzer {
    fn from(tokenizer: TokenizerKind) -> TextAnalyzer {
        use TokenizerKind::*;

        match tokenizer {
            Simple => TextAnalyzer::from(SimpleTokenizer)
                .filter(RemoveLongFilter::limit(40))
                .filter(LowerCaser),
            Ngram => TextAnalyzer::from(NgramTokenizer::new(2, 8, false)).filter(LowerCaser),
            Whitespace => TextAnalyzer::from(WhitespaceTokenizer).filter(LowerCaser),
            #[cfg(feature = "search-lindera")]
            Lindera => {
                TextAnalyzer::from(LinderaTokenizer::new("decompose", "")).filter(LowerCaser)
            }
        }
    }
}

/// Tokenize the text by splitting on whitespaces. Pretty much a copy of Tantivy's SimpleTokenizer,
/// but not splitting on punctuation
#[derive(Clone)]
pub struct WhitespaceTokenizer;

pub struct WhitespaceTokenStream<'a> {
    text: &'a str,
    chars: CharIndices<'a>,
    token: Token,
}

impl Tokenizer for WhitespaceTokenizer {
    fn token_stream<'a>(&self, text: &'a str) -> BoxTokenStream<'a> {
        BoxTokenStream::from(WhitespaceTokenStream {
            text,
            chars: text.char_indices(),
            token: Token::default(),
        })
    }
}
impl<'a> WhitespaceTokenStream<'a> {
    // search for the end of the current token.
    fn search_token_end(&mut self) -> usize {
        (&mut self.chars)
            .filter(|&(_, ref c)| c.is_whitespace())
            .map(|(offset, _)| offset)
            .next()
            .unwrap_or_else(|| self.text.len())
    }
}

impl<'a> TokenStream for WhitespaceTokenStream<'a> {
    fn advance(&mut self) -> bool {
        self.token.text.clear();
        self.token.position = self.token.position.wrapping_add(1);

        loop {
            match self.chars.next() {
                Some((offset_from, c)) => {
                    if !c.is_whitespace() {
                        let offset_to = self.search_token_end();
                        self.token.offset_from = offset_from;
                        self.token.offset_to = offset_to;
                        self.token.text.push_str(&self.text[offset_from..offset_to]);
                        return true;
                    }
                }
                None => {
                    return false;
                }
            }
        }
    }

    fn token(&self) -> &Token {
        &self.token
    }

    fn token_mut(&mut self) -> &mut Token {
        &mut self.token
    }
}
