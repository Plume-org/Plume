use std::str::CharIndices;
use tantivy::tokenizer::{Token, TokenStream, Tokenizer};

/// Tokenize the text by splitting on whitespaces. Pretty much a copy of Tantivy's SimpleTokenizer,
/// but not splitting on punctuation
#[derive(Clone)]
pub struct WhitespaceTokenizer;

pub struct WhitespaceTokenStream<'a> {
    text: &'a str,
    chars: CharIndices<'a>,
    token: Token,
}

impl<'a> Tokenizer<'a> for WhitespaceTokenizer {
    type TokenStreamImpl = WhitespaceTokenStream<'a>;

    fn token_stream(&self, text: &'a str) -> Self::TokenStreamImpl {
        WhitespaceTokenStream {
            text,
            chars: text.char_indices(),
            token: Token::default(),
        }
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
