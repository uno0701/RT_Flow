using RT.Document;
using RT.Document.Models;
using Xunit;

namespace RT.Document.Tests;

public class DocumentProcessorTests
{
    // -----------------------------------------------------------------------
    // Tokenizer tests
    // -----------------------------------------------------------------------

    [Fact]
    public void Tokenizer_EmptyString_ReturnsEmptyList()
    {
        var tokens = Tokenizer.Tokenize(string.Empty);
        Assert.Empty(tokens);
    }

    [Fact]
    public void Tokenizer_SimpleWords_ReturnsWordTokens()
    {
        var tokens = Tokenizer.Tokenize("hello world");
        var words = tokens.Where(t => t.Kind == TokenKind.Word).ToList();
        Assert.Equal(2, words.Count);
        Assert.Equal("hello", words[0].Text);
        Assert.Equal("world", words[1].Text);
    }

    [Fact]
    public void Tokenizer_WordsHaveCorrectOffsets()
    {
        var tokens = Tokenizer.Tokenize("hello world");
        var words = tokens.Where(t => t.Kind == TokenKind.Word).ToList();
        Assert.Equal(0, words[0].Offset);
        Assert.Equal(6, words[1].Offset);
    }

    [Fact]
    public void Tokenizer_NumberToken()
    {
        var tokens = Tokenizer.Tokenize("Pay 1,000.00 dollars");
        var numbers = tokens.Where(t => t.Kind == TokenKind.Number).ToList();
        Assert.Single(numbers);
        Assert.Equal("1,000.00", numbers[0].Text);
    }

    [Fact]
    public void Tokenizer_PunctuationToken()
    {
        var tokens = Tokenizer.Tokenize("hello, world.");
        var puncts = tokens.Where(t => t.Kind == TokenKind.Punctuation).ToList();
        Assert.Contains(puncts, p => p.Text == ",");
        Assert.Contains(puncts, p => p.Text == ".");
    }

    [Fact]
    public void Tokenizer_DefinedTermInDoubleQuotes()
    {
        var tokens = Tokenizer.Tokenize("the \"Agreement\" means");
        var defined = tokens.Where(t => t.Kind == TokenKind.DefinedTerm).ToList();
        Assert.Single(defined);
        Assert.Equal("\"Agreement\"", defined[0].Text);
    }

    [Fact]
    public void Tokenizer_WhitespaceToken()
    {
        var tokens = Tokenizer.Tokenize("a b");
        var ws = tokens.Where(t => t.Kind == TokenKind.Whitespace).ToList();
        Assert.Single(ws);
    }

    [Fact]
    public void Tokenizer_NormalizeConvertsToLowercase()
    {
        var normalized = Tokenizer.Normalize("HELLO");
        Assert.Equal("hello", normalized);
    }

    [Fact]
    public void Tokenizer_NormalizeCollapsesWhitespace()
    {
        var normalized = Tokenizer.Normalize("  hello   world  ");
        Assert.Equal("hello world", normalized);
    }

    [Fact]
    public void Tokenizer_NormalizeStripsQuotesFromDefinedTerm()
    {
        var normalized = Tokenizer.Normalize("\"Agreement\"");
        Assert.Equal("agreement", normalized);
    }

    // -----------------------------------------------------------------------
    // SHA-256 tests (must match Rust implementation)
    // -----------------------------------------------------------------------

    [Fact]
    public void Sha256_EmptyString_MatchesKnownHash()
    {
        // SHA-256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        var result = DocxParser.Sha256Hex(string.Empty);
        Assert.Equal("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855", result);
    }

    [Fact]
    public void Sha256_KnownInput_MatchesExpected()
    {
        // SHA-256("hello") = 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
        var result = DocxParser.Sha256Hex("hello");
        Assert.Equal("2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824", result);
    }

    [Fact]
    public void Sha256_ProducesLowercaseHex()
    {
        var result = DocxParser.Sha256Hex("test");
        Assert.Equal(result.ToLowerInvariant(), result);
    }

    [Fact]
    public void Sha256_Produces64CharHex()
    {
        var result = DocxParser.Sha256Hex("anything");
        Assert.Equal(64, result.Length);
    }

    // -----------------------------------------------------------------------
    // Anchor signature tests
    // -----------------------------------------------------------------------

    [Fact]
    public void AnchorSignature_UsesFirst128CharsOfText()
    {
        // Create text longer than 128 chars and one exactly 128 chars
        var longText = new string('a', 200);
        var shortText = new string('a', 128);

        var sig1 = DocxParser.ComputeAnchorSignature(BlockType.Paragraph, "1.", longText);
        var sig2 = DocxParser.ComputeAnchorSignature(BlockType.Paragraph, "1.", shortText);

        // Both should produce the same signature (uses first 128 chars)
        Assert.Equal(sig1, sig2);
    }

    [Fact]
    public void AnchorSignature_DifferentType_ProducesDifferentSignature()
    {
        var sig1 = DocxParser.ComputeAnchorSignature(BlockType.Paragraph, "1.", "same text");
        var sig2 = DocxParser.ComputeAnchorSignature(BlockType.Section, "1.", "same text");
        Assert.NotEqual(sig1, sig2);
    }

    [Fact]
    public void AnchorSignature_DifferentPath_ProducesDifferentSignature()
    {
        var sig1 = DocxParser.ComputeAnchorSignature(BlockType.Clause, "1.", "same text");
        var sig2 = DocxParser.ComputeAnchorSignature(BlockType.Clause, "2.", "same text");
        Assert.NotEqual(sig1, sig2);
    }

    [Fact]
    public void AnchorSignature_SameInputs_ProducesSameSignature()
    {
        var sig1 = DocxParser.ComputeAnchorSignature(BlockType.Clause, "1.2(a)", "The Borrower shall");
        var sig2 = DocxParser.ComputeAnchorSignature(BlockType.Clause, "1.2(a)", "The Borrower shall");
        Assert.Equal(sig1, sig2);
    }

    [Fact]
    public void AnchorSignature_KnownValue_MatchesExpected()
    {
        // paragraph|1.|hello  → SHA256("paragraph|1.|hello")
        // SHA256("paragraph|1.|hello") = pre-computed
        var input = "paragraph|1.|hello";
        var expected = DocxParser.Sha256Hex(input);
        var sig = DocxParser.ComputeAnchorSignature(BlockType.Paragraph, "1.", "hello");
        Assert.Equal(expected, sig);
    }

    // -----------------------------------------------------------------------
    // Clause hash tests
    // -----------------------------------------------------------------------

    [Fact]
    public void ClauseHash_IsSimplySha256OfCanonicalText()
    {
        var text = "The Borrower shall repay the Loan.";
        var expected = DocxParser.Sha256Hex(text);
        var actual = DocxParser.ComputeClauseHash(text);
        Assert.Equal(expected, actual);
    }

    // -----------------------------------------------------------------------
    // Canonical text normalization tests
    // -----------------------------------------------------------------------

    [Fact]
    public void NormalizeCanonical_CollapsesWhitespace()
    {
        var result = DocxParser.NormalizeCanonical("hello   world\t\nfoo");
        Assert.Equal("hello world foo", result);
    }

    [Fact]
    public void NormalizeCanonical_TrimsLeadingTrailingWhitespace()
    {
        var result = DocxParser.NormalizeCanonical("  hello world  ");
        Assert.Equal("hello world", result);
    }

    [Fact]
    public void NormalizeCanonical_EmptyInput_ReturnsEmpty()
    {
        Assert.Equal(string.Empty, DocxParser.NormalizeCanonical(string.Empty));
    }
}
