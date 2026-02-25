using RT.Document;
using Xunit;

namespace RT.Document.Tests;

public class DocumentProcessorTests
{
    [Fact]
    public void Placeholder_Test_Passes()
    {
        // Arrange
        var processor = new DocumentProcessor();

        // Act / Assert
        Assert.NotNull(processor);
    }
}
