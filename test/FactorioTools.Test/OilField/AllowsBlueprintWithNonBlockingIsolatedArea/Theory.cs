namespace Knapcode.FactorioTools.OilField;

public class AllowsBlueprintWithNonBlockingIsolatedArea : BasePlannerTest
{
    [Theory]
    [MemberData(nameof(AllPipeStrategiesTestData))]
    public async Task Execute(PipeStrategy strategy)
    {
        // Arrange
        var options = OilFieldOptions.ForMediumElectricPole;
        options.PipeStrategies = new List<PipeStrategy> { strategy };
        var blueprintString = "0eNqV1UluhDAQBdC71NoLXLYBc5UoatG0FTlpDGKIghB3D9MiAxKfJWAehVXfNdL92bu68aGjbCRfVKGl7GWk1r+F/LncC3npKKO6L+v3vPggQd1QL3d850qaBPnwcF+UyelVkAud77zbjPViuIW+vLtmXiAOrLpq5xeqsHxpRlgLGualanYfvnHF9iyaxD+OAU4xzCmES2BOI1y0cfFvTh9w5kJ1ABcDnNy55PxnE4DT8cal51yKNMpenT3nLLJ3CuZkhGze5nEEeBdywRLwGG89yLuQDGbAQ6LBe31A0qTB24U14CHhUBr3oHREuIfEQ1+oD8kH7/0MHAaM5EPZQ+/orGIoH2b1FNDPfGFwQB6Uj+30U+avN8/MdY5mPwaxoE/XtOsCE7PV1hpjZKpjPU3fXdaDKQ==";

        var blueprint = ParseBlueprint.Execute(blueprintString);

        // Act
        var result = Planner.Execute(options, blueprint);

        // Assert
#if USE_VERIFY
        await Verify(GetGridString(result))
            .UseTypeName("Theory")
            .UseMethodName("E")
            .UseTextForParameters($"{strategy}");
#else
        await Task.Yield();
#endif
    }
}
