using RT.Transform;
using Xunit;

namespace RT.Transform.Tests;

public class TransformPipelineTests
{
    [Fact]
    public void Placeholder_Test_Passes()
    {
        // Arrange
        var pipeline = new TransformPipeline();

        // Act / Assert
        Assert.NotNull(pipeline);
    }
}
