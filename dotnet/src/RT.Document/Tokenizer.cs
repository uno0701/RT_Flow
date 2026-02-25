using System.Text;
using System.Text.RegularExpressions;
using RT.Document.Models;

namespace RT.Document;

/// <summary>
/// Tokenizes text into a hybrid word + punctuation token stream.
/// Mirrors the behavior of the Rust tokenizer in rt-core.
/// </summary>
public static class Tokenizer
{
    // Defined term pattern: text enclosed in double-quotes or "Capitalised Word(s)"
    private static readonly Regex DefinedTermPattern = new(
        @"""[^""]+""",
        RegexOptions.Compiled);

    // Date reference pattern (simple)
    private static readonly Regex DatePattern = new(
        @"\b(?:\d{1,2}[-/]\d{1,2}[-/]\d{2,4}|\d{4}[-/]\d{2}[-/]\d{2}|"
        + @"(?:January|February|March|April|May|June|July|August|September|October|November|December)\s+\d{1,2},?\s+\d{4})\b",
        RegexOptions.Compiled | RegexOptions.IgnoreCase);

    // Number pattern
    private static readonly Regex NumberPattern = new(
        @"\b\d[\d,]*(?:\.\d+)?%?\b",
        RegexOptions.Compiled);

    // Word with optional apostrophe (contractions, possessives)
    private static readonly Regex WordPattern = new(
        @"[A-Za-z\u00C0-\u024F]+(?:'[A-Za-z]+)?",
        RegexOptions.Compiled);

    /// <summary>
    /// Tokenize text into a list of Token objects with offsets.
    /// </summary>
    public static List<Token> Tokenize(string text)
    {
        if (string.IsNullOrEmpty(text))
            return new List<Token>();

        var tokens = new List<Token>();
        var pos = 0;

        while (pos < text.Length)
        {
            var ch = text[pos];

            // Whitespace
            if (char.IsWhiteSpace(ch))
            {
                var start = pos;
                while (pos < text.Length && char.IsWhiteSpace(text[pos]))
                    pos++;
                var wsText = text[start..pos];
                tokens.Add(new Token(wsText, TokenKind.Whitespace, " ", start));
                continue;
            }

            // Try defined term: "quoted text"
            if (ch == '"' || ch == '\u201C' || ch == '\u201D')
            {
                var closeChar = ch == '"' ? '"' : '\u201D';
                // Normalize open/close quotes
                var endPos = pos + 1;
                while (endPos < text.Length && text[endPos] != '"' && text[endPos] != '\u201D' && text[endPos] != closeChar)
                    endPos++;
                if (endPos < text.Length)
                {
                    endPos++; // include closing quote
                    var termText = text[pos..endPos];
                    tokens.Add(new Token(termText, TokenKind.DefinedTerm, Normalize(termText), pos));
                    pos = endPos;
                    continue;
                }
            }

            // Number
            if (char.IsDigit(ch))
            {
                var start = pos;
                // Consume digits, commas (thousands separators), dots (decimals), percent
                while (pos < text.Length && (char.IsDigit(text[pos]) || text[pos] == ',' || text[pos] == '.' || text[pos] == '%'))
                    pos++;
                var numText = text[start..pos];
                tokens.Add(new Token(numText, TokenKind.Number, Normalize(numText), start));
                continue;
            }

            // Word (letters including accented)
            if (char.IsLetter(ch))
            {
                var start = pos;
                while (pos < text.Length && (char.IsLetter(text[pos]) || (text[pos] == '\'' && pos + 1 < text.Length && char.IsLetter(text[pos + 1]))))
                    pos++;
                var wordText = text[start..pos];
                // Check if it looks like a defined term (Title Case multi-word will be checked at the call site)
                tokens.Add(new Token(wordText, TokenKind.Word, Normalize(wordText), start));
                continue;
            }

            // Punctuation / symbol (single character)
            {
                var punct = text[pos..++pos];
                tokens.Add(new Token(punct, TokenKind.Punctuation, punct, pos - 1));
            }
        }

        return tokens;
    }

    /// <summary>
    /// Normalize a token: lowercase, trim, collapse internal whitespace.
    /// Matches the Rust normalize behavior.
    /// </summary>
    public static string Normalize(string token)
    {
        if (string.IsNullOrEmpty(token)) return string.Empty;

        // Strip surrounding quotes for defined terms
        var stripped = token.Trim();
        if ((stripped.StartsWith('"') && stripped.EndsWith('"')) ||
            (stripped.StartsWith('\u201C') && stripped.EndsWith('\u201D')))
        {
            stripped = stripped[1..^1];
        }

        // Lowercase and collapse whitespace
        var sb = new StringBuilder(stripped.Length);
        var prevWs = false;
        foreach (var c in stripped.ToLowerInvariant())
        {
            if (char.IsWhiteSpace(c))
            {
                if (!prevWs) sb.Append(' ');
                prevWs = true;
            }
            else
            {
                sb.Append(c);
                prevWs = false;
            }
        }
        return sb.ToString().Trim();
    }
}
