using RT.Document.Models;
using RT.Transform;
using Xunit;

namespace RT.Transform.Tests;

public class TransformPipelineTests
{
    // -----------------------------------------------------------------------
    // Canonical text normalization
    // -----------------------------------------------------------------------

    [Fact]
    public void NormalizeCanonical_CollapsesMultipleSpaces()
    {
        var result = TransformPipeline.NormalizeCanonical("hello   world");
        Assert.Equal("hello world", result);
    }

    [Fact]
    public void NormalizeCanonical_CollapsesTabs()
    {
        var result = TransformPipeline.NormalizeCanonical("hello\tworld");
        Assert.Equal("hello world", result);
    }

    [Fact]
    public void NormalizeCanonical_CollapsesNewlines()
    {
        var result = TransformPipeline.NormalizeCanonical("hello\nworld");
        Assert.Equal("hello world", result);
    }

    [Fact]
    public void NormalizeCanonical_TrimsEnds()
    {
        var result = TransformPipeline.NormalizeCanonical("  hello world  ");
        Assert.Equal("hello world", result);
    }

    [Fact]
    public void NormalizeCanonical_EmptyString()
    {
        Assert.Equal(string.Empty, TransformPipeline.NormalizeCanonical(string.Empty));
    }

    [Fact]
    public void NormalizeCanonical_PreservesNonWhitespaceContent()
    {
        var text = "The Borrower shall repay the Loan in full.";
        var result = TransformPipeline.NormalizeCanonical(text);
        Assert.Equal(text, result);
    }

    [Fact]
    public void NormalizeCanonical_StripsNumberingPrefix_Decimal()
    {
        var result = TransformPipeline.NormalizeCanonical("1. The Borrower shall");
        Assert.Equal("The Borrower shall", result);
    }

    [Fact]
    public void NormalizeCanonical_StripsNumberingPrefix_ParenLetter()
    {
        var result = TransformPipeline.NormalizeCanonical("(a) repay the amount");
        Assert.Equal("repay the amount", result);
    }

    // -----------------------------------------------------------------------
    // SHA-256 hash computation (must match Rust output)
    // -----------------------------------------------------------------------

    [Fact]
    public void Sha256Hex_EmptyString_MatchesKnownHash()
    {
        // SHA-256 of empty string is a well-known constant
        var result = TransformPipeline.Sha256Hex(string.Empty);
        Assert.Equal("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855", result);
    }

    [Fact]
    public void Sha256Hex_HelloString_MatchesKnownHash()
    {
        var result = TransformPipeline.Sha256Hex("hello");
        Assert.Equal("2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824", result);
    }

    [Fact]
    public void Sha256Hex_IsDeterministic()
    {
        var r1 = TransformPipeline.Sha256Hex("deterministic input");
        var r2 = TransformPipeline.Sha256Hex("deterministic input");
        Assert.Equal(r1, r2);
    }

    [Fact]
    public void Sha256Hex_DifferentInputs_ProduceDifferentHashes()
    {
        var r1 = TransformPipeline.Sha256Hex("input one");
        var r2 = TransformPipeline.Sha256Hex("input two");
        Assert.NotEqual(r1, r2);
    }

    [Fact]
    public void Sha256Hex_ResultIsLowercaseHex64Chars()
    {
        var result = TransformPipeline.Sha256Hex("any text");
        Assert.Equal(64, result.Length);
        Assert.Equal(result.ToLowerInvariant(), result);
        Assert.True(result.All(c => (c >= '0' && c <= '9') || (c >= 'a' && c <= 'f')));
    }

    // -----------------------------------------------------------------------
    // Anchor signature computation
    // -----------------------------------------------------------------------

    [Fact]
    public void AnchorSignature_KnownFormat_MatchesExpected()
    {
        // The format is SHA256( "type|path|first128chars" )
        var expected = TransformPipeline.Sha256Hex("paragraph|1.2|The Borrower shall");
        var actual = TransformPipeline.ComputeAnchorSignature(BlockType.Paragraph, "1.2", "The Borrower shall");
        Assert.Equal(expected, actual);
    }

    [Fact]
    public void AnchorSignature_TextOver128Chars_UsesFirst128Only()
    {
        var text200 = new string('x', 200);
        var text128 = new string('x', 128);

        var sig1 = TransformPipeline.ComputeAnchorSignature(BlockType.Clause, "1.", text200);
        var sig2 = TransformPipeline.ComputeAnchorSignature(BlockType.Clause, "1.", text128);

        Assert.Equal(sig1, sig2);
    }

    [Fact]
    public void AnchorSignature_DifferentBlockTypes_ProduceDifferentSignatures()
    {
        var sig1 = TransformPipeline.ComputeAnchorSignature(BlockType.Section, "1.", "text");
        var sig2 = TransformPipeline.ComputeAnchorSignature(BlockType.Clause, "1.", "text");
        Assert.NotEqual(sig1, sig2);
    }

    // -----------------------------------------------------------------------
    // Clause hash computation
    // -----------------------------------------------------------------------

    [Fact]
    public void ClauseHash_IsSha256OfCanonicalText()
    {
        var text = "The Borrower shall repay the Loan.";
        var expected = TransformPipeline.Sha256Hex(text);
        var actual = TransformPipeline.ComputeClauseHash(text);
        Assert.Equal(expected, actual);
    }

    [Fact]
    public void ClauseHash_EmptyText()
    {
        var expected = TransformPipeline.Sha256Hex(string.Empty);
        var actual = TransformPipeline.ComputeClauseHash(string.Empty);
        Assert.Equal(expected, actual);
    }

    // -----------------------------------------------------------------------
    // TransformPipeline instantiation
    // -----------------------------------------------------------------------

    [Fact]
    public void TransformPipeline_CanBeInstantiated()
    {
        var pipeline = new TransformPipeline();
        Assert.NotNull(pipeline);
    }
}
