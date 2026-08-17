using Knapcode.FactorioTools.Data;

namespace Knapcode.FactorioTools.OilField;

public class PlannerIssueTest : BasePlannerTest
{
    [Fact]
    public async Task CanPlanCollinearConnectedCenters()
    {
        // Arrange
        var options = OilFieldOptions.ForSmallElectricPole;
        options.AddBeacons = false;
        options.ValidateSolution = true;
        var blueprint = new Blueprint
        {
            Entities = new[]
            {
                new Entity
                {
                    Name = EntityNames.Vanilla.Pumpjack,
                    Position = new Position { X = -23.5f, Y = -37.5f },
                },
                new Entity
                {
                    Name = EntityNames.Vanilla.Pumpjack,
                    Position = new Position { X = -21.5f, Y = -34.5f },
                },
                new Entity
                {
                    Name = EntityNames.Vanilla.Pumpjack,
                    Position = new Position { X = -19.5f, Y = -31.5f },
                },
            }
        };

        // Act
        var result = Planner.Execute(options, blueprint);

        // Assert
#if USE_VERIFY
        await Verify(GetGridString(result));
#else
        await Task.Yield();
#endif
    }

    [Fact]
    public async Task AllowsManyTurnsAroundAvoidLocations()
    {
        // Arrange
        var options = OilFieldOptions.ForSmallElectricPole;
        options.ValidateSolution = true;
        var blueprint = new Blueprint
        {
            Entities = new[]
            {
                new Entity
                {
                    Name = EntityNames.Vanilla.Pumpjack,
                    Position = new Position { X = 35.5f, Y = -21.5f },
                },
                new Entity
                {
                    Name = EntityNames.Vanilla.Pumpjack,
                    Position = new Position { X = 35.5f, Y = -13.5f },
                },
                new Entity
                {
                    Name = EntityNames.Vanilla.Pumpjack,
                    Position = new Position { X = 40.5f, Y = -33.5f },
                },
                new Entity
                {
                    Name = EntityNames.Vanilla.Pumpjack,
                    Position = new Position { X = 48.5f, Y = -38.5f },
                },
                new Entity
                {
                    Name = EntityNames.Vanilla.Pumpjack,
                    Position = new Position { X = 51.5f, Y = -27.5f },
                },

            }
        };
        var avoid = new AvoidLocation[]
        {
              new AvoidLocation(33.5f, -10.5f),
              new AvoidLocation(34.5f, -10.5f),
              new AvoidLocation(35.5f, -10.5f),
              new AvoidLocation(36.5f, -11.5f),
              new AvoidLocation(36.5f, -10.5f),
              new AvoidLocation(37.5f, -11.5f),
              new AvoidLocation(37.5f, -10.5f),
              new AvoidLocation(38.5f, -17.5f),
              new AvoidLocation(38.5f, -16.5f),
              new AvoidLocation(38.5f, -15.5f),
              new AvoidLocation(38.5f, -14.5f),
              new AvoidLocation(38.5f, -11.5f),
              new AvoidLocation(38.5f, -10.5f),
              new AvoidLocation(39.5f, -18.5f),
              new AvoidLocation(39.5f, -17.5f),
              new AvoidLocation(39.5f, -16.5f),
              new AvoidLocation(39.5f, -15.5f),
              new AvoidLocation(39.5f, -14.5f),
              new AvoidLocation(39.5f, -13.5f),
              new AvoidLocation(39.5f, -10.5f),
              new AvoidLocation(40.5f, -20.5f),
              new AvoidLocation(40.5f, -19.5f),
              new AvoidLocation(40.5f, -18.5f),
              new AvoidLocation(40.5f, -17.5f),
              new AvoidLocation(40.5f, -16.5f),
              new AvoidLocation(40.5f, -15.5f),
              new AvoidLocation(40.5f, -14.5f),
              new AvoidLocation(40.5f, -13.5f),
              new AvoidLocation(40.5f, -12.5f),
              new AvoidLocation(40.5f, -11.5f),
              new AvoidLocation(40.5f, -10.5f),
              new AvoidLocation(41.5f, -26.5f),
              new AvoidLocation(41.5f, -25.5f),
              new AvoidLocation(41.5f, -24.5f),
              new AvoidLocation(41.5f, -23.5f),
              new AvoidLocation(41.5f, -22.5f),
              new AvoidLocation(41.5f, -21.5f),
              new AvoidLocation(41.5f, -20.5f),
              new AvoidLocation(41.5f, -19.5f),
              new AvoidLocation(41.5f, -18.5f),
              new AvoidLocation(41.5f, -17.5f),
              new AvoidLocation(41.5f, -16.5f),
              new AvoidLocation(41.5f, -15.5f),
              new AvoidLocation(41.5f, -14.5f),
              new AvoidLocation(41.5f, -13.5f),
              new AvoidLocation(41.5f, -12.5f),
              new AvoidLocation(41.5f, -11.5f),
              new AvoidLocation(41.5f, -10.5f),
              new AvoidLocation(42.5f, -39.5f),
              new AvoidLocation(42.5f, -38.5f),
              new AvoidLocation(42.5f, -37.5f),
              new AvoidLocation(42.5f, -27.5f),
              new AvoidLocation(42.5f, -26.5f),
              new AvoidLocation(42.5f, -25.5f),
              new AvoidLocation(42.5f, -24.5f),
              new AvoidLocation(42.5f, -23.5f),
              new AvoidLocation(42.5f, -22.5f),
              new AvoidLocation(42.5f, -21.5f),
              new AvoidLocation(42.5f, -20.5f),
              new AvoidLocation(42.5f, -19.5f),
              new AvoidLocation(42.5f, -18.5f),
              new AvoidLocation(42.5f, -17.5f),
              new AvoidLocation(42.5f, -16.5f),
              new AvoidLocation(42.5f, -15.5f),
              new AvoidLocation(42.5f, -14.5f),
              new AvoidLocation(42.5f, -13.5f),
              new AvoidLocation(42.5f, -11.5f),
              new AvoidLocation(42.5f, -10.5f),
              new AvoidLocation(43.5f, -41.5f),
              new AvoidLocation(43.5f, -40.5f),
              new AvoidLocation(43.5f, -39.5f),
              new AvoidLocation(43.5f, -38.5f),
              new AvoidLocation(43.5f, -37.5f),
              new AvoidLocation(43.5f, -36.5f),
              new AvoidLocation(43.5f, -28.5f),
              new AvoidLocation(43.5f, -27.5f),
              new AvoidLocation(43.5f, -26.5f),
              new AvoidLocation(43.5f, -25.5f),
              new AvoidLocation(43.5f, -24.5f),
              new AvoidLocation(43.5f, -23.5f),
              new AvoidLocation(43.5f, -22.5f),
              new AvoidLocation(43.5f, -21.5f),
              new AvoidLocation(43.5f, -20.5f),
              new AvoidLocation(43.5f, -19.5f),
              new AvoidLocation(43.5f, -18.5f),
              new AvoidLocation(43.5f, -17.5f),
              new AvoidLocation(43.5f, -16.5f),
              new AvoidLocation(43.5f, -15.5f),
              new AvoidLocation(43.5f, -14.5f),
              new AvoidLocation(43.5f, -13.5f),
              new AvoidLocation(43.5f, -12.5f),
              new AvoidLocation(43.5f, -11.5f),
              new AvoidLocation(43.5f, -10.5f),
              new AvoidLocation(44.5f, -41.5f),
              new AvoidLocation(44.5f, -40.5f),
              new AvoidLocation(44.5f, -39.5f),
              new AvoidLocation(44.5f, -38.5f),
              new AvoidLocation(44.5f, -37.5f),
              new AvoidLocation(44.5f, -36.5f),
              new AvoidLocation(44.5f, -30.5f),
              new AvoidLocation(44.5f, -29.5f),
              new AvoidLocation(44.5f, -28.5f),
              new AvoidLocation(44.5f, -27.5f),
              new AvoidLocation(44.5f, -26.5f),
              new AvoidLocation(44.5f, -25.5f),
              new AvoidLocation(44.5f, -24.5f),
              new AvoidLocation(44.5f, -23.5f),
              new AvoidLocation(44.5f, -22.5f),
              new AvoidLocation(44.5f, -21.5f),
              new AvoidLocation(44.5f, -20.5f),
              new AvoidLocation(44.5f, -19.5f),
              new AvoidLocation(44.5f, -18.5f),
              new AvoidLocation(44.5f, -17.5f),
              new AvoidLocation(44.5f, -16.5f),
              new AvoidLocation(44.5f, -15.5f),
              new AvoidLocation(44.5f, -14.5f),
              new AvoidLocation(44.5f, -13.5f),
              new AvoidLocation(44.5f, -12.5f),
              new AvoidLocation(44.5f, -11.5f),
              new AvoidLocation(44.5f, -10.5f),
              new AvoidLocation(45.5f, -41.5f),
              new AvoidLocation(45.5f, -40.5f),
              new AvoidLocation(45.5f, -39.5f),
              new AvoidLocation(45.5f, -38.5f),
              new AvoidLocation(45.5f, -37.5f),
              new AvoidLocation(45.5f, -36.5f),
              new AvoidLocation(45.5f, -33.5f),
              new AvoidLocation(45.5f, -32.5f),
              new AvoidLocation(45.5f, -31.5f),
              new AvoidLocation(45.5f, -30.5f),
              new AvoidLocation(45.5f, -29.5f),
              new AvoidLocation(45.5f, -28.5f),
              new AvoidLocation(45.5f, -27.5f),
              new AvoidLocation(45.5f, -26.5f),
              new AvoidLocation(45.5f, -25.5f),
              new AvoidLocation(45.5f, -24.5f),
              new AvoidLocation(45.5f, -23.5f),
              new AvoidLocation(45.5f, -22.5f),
              new AvoidLocation(45.5f, -21.5f),
              new AvoidLocation(45.5f, -20.5f),
              new AvoidLocation(45.5f, -19.5f),
              new AvoidLocation(45.5f, -18.5f),
              new AvoidLocation(45.5f, -17.5f),
              new AvoidLocation(45.5f, -16.5f),
              new AvoidLocation(45.5f, -15.5f),
              new AvoidLocation(45.5f, -14.5f),
              new AvoidLocation(45.5f, -13.5f),
              new AvoidLocation(45.5f, -12.5f),
              new AvoidLocation(45.5f, -11.5f),
              new AvoidLocation(45.5f, -10.5f),
              new AvoidLocation(46.5f, -33.5f),
              new AvoidLocation(46.5f, -32.5f),
              new AvoidLocation(46.5f, -31.5f),
              new AvoidLocation(46.5f, -30.5f),
              new AvoidLocation(46.5f, -29.5f),
              new AvoidLocation(46.5f, -28.5f),
              new AvoidLocation(46.5f, -27.5f),
              new AvoidLocation(46.5f, -26.5f),
              new AvoidLocation(46.5f, -25.5f),
              new AvoidLocation(46.5f, -24.5f),
              new AvoidLocation(46.5f, -23.5f),
              new AvoidLocation(46.5f, -22.5f),
              new AvoidLocation(46.5f, -21.5f),
              new AvoidLocation(46.5f, -20.5f),
              new AvoidLocation(46.5f, -19.5f),
              new AvoidLocation(46.5f, -18.5f),
              new AvoidLocation(46.5f, -17.5f),
              new AvoidLocation(46.5f, -16.5f),
              new AvoidLocation(46.5f, -15.5f),
              new AvoidLocation(46.5f, -14.5f),
              new AvoidLocation(46.5f, -13.5f),
              new AvoidLocation(46.5f, -12.5f),
              new AvoidLocation(46.5f, -11.5f),
              new AvoidLocation(46.5f, -10.5f),
              new AvoidLocation(47.5f, -31.5f),
              new AvoidLocation(47.5f, -30.5f),
              new AvoidLocation(47.5f, -29.5f),
              new AvoidLocation(47.5f, -25.5f),
              new AvoidLocation(47.5f, -24.5f),
              new AvoidLocation(47.5f, -23.5f),
              new AvoidLocation(47.5f, -22.5f),
              new AvoidLocation(47.5f, -21.5f),
              new AvoidLocation(47.5f, -20.5f),
              new AvoidLocation(47.5f, -19.5f),
              new AvoidLocation(47.5f, -18.5f),
              new AvoidLocation(47.5f, -17.5f),
              new AvoidLocation(47.5f, -16.5f),
              new AvoidLocation(47.5f, -15.5f),
              new AvoidLocation(47.5f, -14.5f),
              new AvoidLocation(47.5f, -13.5f),
              new AvoidLocation(47.5f, -12.5f),
              new AvoidLocation(47.5f, -11.5f),
              new AvoidLocation(47.5f, -10.5f),
              new AvoidLocation(48.5f, -24.5f),
              new AvoidLocation(48.5f, -23.5f),
              new AvoidLocation(48.5f, -22.5f),
              new AvoidLocation(48.5f, -21.5f),
              new AvoidLocation(48.5f, -20.5f),
              new AvoidLocation(48.5f, -19.5f),
              new AvoidLocation(48.5f, -18.5f),
              new AvoidLocation(48.5f, -17.5f),
              new AvoidLocation(48.5f, -16.5f),
              new AvoidLocation(48.5f, -15.5f),
              new AvoidLocation(48.5f, -14.5f),
              new AvoidLocation(48.5f, -13.5f),
              new AvoidLocation(48.5f, -12.5f),
              new AvoidLocation(48.5f, -11.5f),
              new AvoidLocation(48.5f, -10.5f),
              new AvoidLocation(49.5f, -24.5f),
              new AvoidLocation(49.5f, -23.5f),
              new AvoidLocation(49.5f, -22.5f),
              new AvoidLocation(49.5f, -21.5f),
              new AvoidLocation(49.5f, -20.5f),
              new AvoidLocation(49.5f, -19.5f),
              new AvoidLocation(49.5f, -18.5f),
              new AvoidLocation(49.5f, -17.5f),
              new AvoidLocation(49.5f, -16.5f),
              new AvoidLocation(49.5f, -15.5f),
              new AvoidLocation(49.5f, -14.5f),
              new AvoidLocation(49.5f, -13.5f),
              new AvoidLocation(49.5f, -12.5f),
              new AvoidLocation(49.5f, -11.5f),
              new AvoidLocation(49.5f, -10.5f),
              new AvoidLocation(50.5f, -24.5f),
              new AvoidLocation(50.5f, -23.5f),
              new AvoidLocation(50.5f, -22.5f),
              new AvoidLocation(50.5f, -21.5f),
              new AvoidLocation(50.5f, -20.5f),
              new AvoidLocation(50.5f, -19.5f),
              new AvoidLocation(50.5f, -18.5f),
              new AvoidLocation(50.5f, -17.5f),
              new AvoidLocation(50.5f, -16.5f),
              new AvoidLocation(50.5f, -15.5f),
              new AvoidLocation(50.5f, -14.5f),
              new AvoidLocation(50.5f, -13.5f),
              new AvoidLocation(50.5f, -12.5f),
              new AvoidLocation(50.5f, -11.5f),
              new AvoidLocation(50.5f, -10.5f),
              new AvoidLocation(51.5f, -24.5f),
              new AvoidLocation(51.5f, -23.5f),
              new AvoidLocation(51.5f, -22.5f),
              new AvoidLocation(51.5f, -21.5f),
              new AvoidLocation(51.5f, -20.5f),
              new AvoidLocation(51.5f, -19.5f),
              new AvoidLocation(51.5f, -18.5f),
              new AvoidLocation(51.5f, -17.5f),
              new AvoidLocation(51.5f, -14.5f),
              new AvoidLocation(51.5f, -13.5f),
              new AvoidLocation(51.5f, -12.5f),
              new AvoidLocation(51.5f, -11.5f),
              new AvoidLocation(51.5f, -10.5f),
              new AvoidLocation(52.5f, -24.5f),
              new AvoidLocation(52.5f, -23.5f),
              new AvoidLocation(52.5f, -22.5f),
              new AvoidLocation(52.5f, -21.5f),
              new AvoidLocation(52.5f, -20.5f),
              new AvoidLocation(52.5f, -19.5f),
              new AvoidLocation(52.5f, -18.5f),
              new AvoidLocation(53.5f, -24.5f),
              new AvoidLocation(53.5f, -23.5f),
              new AvoidLocation(53.5f, -22.5f),
              new AvoidLocation(53.5f, -21.5f),
              new AvoidLocation(53.5f, -20.5f),
              new AvoidLocation(53.5f, -19.5f),
              new AvoidLocation(53.5f, -18.5f),
              new AvoidLocation(53.5f, -17.5f),
              new AvoidLocation(54.5f, -29.5f),
              new AvoidLocation(54.5f, -28.5f),
              new AvoidLocation(54.5f, -27.5f),
              new AvoidLocation(54.5f, -25.5f),
              new AvoidLocation(54.5f, -24.5f),
              new AvoidLocation(54.5f, -23.5f),
              new AvoidLocation(54.5f, -22.5f),
              new AvoidLocation(54.5f, -21.5f),
              new AvoidLocation(54.5f, -20.5f),
              new AvoidLocation(54.5f, -19.5f),
              new AvoidLocation(54.5f, -18.5f),
              new AvoidLocation(54.5f, -17.5f),
        };

        // Act
        var result = Planner.Execute(options, blueprint, avoid);

        // Assert
#if USE_VERIFY
        await Verify(GetGridString(result));
#else
        await Task.Yield();
#endif
    }

    [Fact]
    public async Task CanPlanSinglePumpjackSurrounded()
    {
        // Arrange
        var options = OilFieldOptions.ForMediumElectricPole;
        options.ValidateSolution = true;
        var blueprint = new Blueprint
        {
            Entities = new[]
            {
                new Entity
                {
                    Name = EntityNames.Vanilla.Pumpjack,
                    Position = new Position { X = 43.5f, Y = -3.5f },
                }
            }
        };
        var avoid = new AvoidLocation[]
        {
            new AvoidLocation(40.5f, -2.5f),
            new AvoidLocation(40.5f, -3.5f),
            new AvoidLocation(40.5f, -4.5f),
            new AvoidLocation(40.5f, -5.5f),
            new AvoidLocation(40.5f, -6.5f),
            new AvoidLocation(41.5f, -2.5f),
            new AvoidLocation(41.5f, -3.5f),
            new AvoidLocation(41.5f, -4.5f),
            new AvoidLocation(41.5f, -5.5f),
            new AvoidLocation(41.5f, -6.5f),
            new AvoidLocation(42.5f, -6.5f),
            new AvoidLocation(43.5f, -6.5f),
            new AvoidLocation(44.5f, -5.5f),
            new AvoidLocation(44.5f, -6.5f),
            new AvoidLocation(45.5f, -4.5f),
            new AvoidLocation(45.5f, -5.5f),
            new AvoidLocation(45.5f, -6.5f),
            new AvoidLocation(46.5f, -0.5f),
            new AvoidLocation(46.5f, -3.5f),
            new AvoidLocation(46.5f, -4.5f),
            new AvoidLocation(46.5f, -5.5f),
            new AvoidLocation(46.5f, -6.5f),
        };

        // Act
        var result = Planner.Execute(options, blueprint, avoid);

        // Assert
#if USE_VERIFY
        await Verify(GetGridString(result));
#else
        await Task.Yield();
#endif
    }

    [Fact]
    public async Task CanPlanTwoPumpjacksWithLotsOfAvoids()
    {
        // Arrange
        var options = OilFieldOptions.ForSmallElectricPole;
        options.AddBeacons = false;
        options.ValidateSolution = true;
        var blueprint = new Blueprint
        {
            Entities = new[]
            {
                new Entity
                {
                    Name = EntityNames.Vanilla.Pumpjack,
                    Position = new Position { X = 35.5f, Y = -13.5f },
                },
                new Entity
                {
                    Name = EntityNames.Vanilla.Pumpjack,
                    Position = new Position { X = 43.5f, Y = -3.5f },
                }
            }
        };
        var avoid = new AvoidLocation[]
        {
            new AvoidLocation(32.5f, -9.5f),
            new AvoidLocation(32.5f, -8.5f),
            new AvoidLocation(33.5f, -10.5f),
            new AvoidLocation(33.5f, -9.5f),
            new AvoidLocation(33.5f, -8.5f),
            new AvoidLocation(33.5f, -7.5f),
            new AvoidLocation(33.5f, -6.5f),
            new AvoidLocation(34.5f, -10.5f),
            new AvoidLocation(34.5f, -9.5f),
            new AvoidLocation(34.5f, -8.5f),
            new AvoidLocation(34.5f, -7.5f),
            new AvoidLocation(34.5f, -6.5f),
            new AvoidLocation(34.5f, -5.5f),
            new AvoidLocation(34.5f, -4.5f),
            new AvoidLocation(34.5f, -3.5f),
            new AvoidLocation(35.5f, -10.5f),
            new AvoidLocation(35.5f, -9.5f),
            new AvoidLocation(35.5f, -8.5f),
            new AvoidLocation(35.5f, -7.5f),
            new AvoidLocation(35.5f, -6.5f),
            new AvoidLocation(35.5f, -5.5f),
            new AvoidLocation(35.5f, -4.5f),
            new AvoidLocation(35.5f, -3.5f),
            new AvoidLocation(35.5f, -2.5f),
            new AvoidLocation(36.5f, -11.5f),
            new AvoidLocation(36.5f, -10.5f),
            new AvoidLocation(36.5f, -9.5f),
            new AvoidLocation(36.5f, -8.5f),
            new AvoidLocation(36.5f, -7.5f),
            new AvoidLocation(36.5f, -6.5f),
            new AvoidLocation(36.5f, -5.5f),
            new AvoidLocation(36.5f, -4.5f),
            new AvoidLocation(36.5f, -3.5f),
            new AvoidLocation(36.5f, -2.5f),
            new AvoidLocation(37.5f, -11.5f),
            new AvoidLocation(37.5f, -10.5f),
            new AvoidLocation(37.5f, -9.5f),
            new AvoidLocation(37.5f, -8.5f),
            new AvoidLocation(37.5f, -7.5f),
            new AvoidLocation(37.5f, -6.5f),
            new AvoidLocation(37.5f, -5.5f),
            new AvoidLocation(37.5f, -4.5f),
            new AvoidLocation(37.5f, -3.5f),
            new AvoidLocation(37.5f, -1.5f),
            new AvoidLocation(37.5f, -0.5f),
            new AvoidLocation(38.5f, -16.5f),
            new AvoidLocation(38.5f, -15.5f),
            new AvoidLocation(38.5f, -14.5f),
            new AvoidLocation(38.5f, -11.5f),
            new AvoidLocation(38.5f, -10.5f),
            new AvoidLocation(38.5f, -9.5f),
            new AvoidLocation(38.5f, -8.5f),
            new AvoidLocation(38.5f, -7.5f),
            new AvoidLocation(38.5f, -6.5f),
            new AvoidLocation(38.5f, -5.5f),
            new AvoidLocation(38.5f, -4.5f),
            new AvoidLocation(38.5f, -3.5f),
            new AvoidLocation(38.5f, -1.5f),
            new AvoidLocation(38.5f, -0.5f),
            new AvoidLocation(39.5f, -16.5f),
            new AvoidLocation(39.5f, -15.5f),
            new AvoidLocation(39.5f, -14.5f),
            new AvoidLocation(39.5f, -13.5f),
            new AvoidLocation(39.5f, -10.5f),
            new AvoidLocation(39.5f, -9.5f),
            new AvoidLocation(39.5f, -8.5f),
            new AvoidLocation(39.5f, -7.5f),
            new AvoidLocation(39.5f, -6.5f),
            new AvoidLocation(39.5f, -5.5f),
            new AvoidLocation(39.5f, -4.5f),
            new AvoidLocation(39.5f, -3.5f),
            new AvoidLocation(39.5f, -1.5f),
            new AvoidLocation(39.5f, -0.5f),
            new AvoidLocation(40.5f, -16.5f),
            new AvoidLocation(40.5f, -15.5f),
            new AvoidLocation(40.5f, -14.5f),
            new AvoidLocation(40.5f, -13.5f),
            new AvoidLocation(40.5f, -12.5f),
            new AvoidLocation(40.5f, -11.5f),
            new AvoidLocation(40.5f, -10.5f),
            new AvoidLocation(40.5f, -9.5f),
            new AvoidLocation(40.5f, -8.5f),
            new AvoidLocation(40.5f, -7.5f),
            new AvoidLocation(40.5f, -6.5f),
            new AvoidLocation(40.5f, -5.5f),
            new AvoidLocation(40.5f, -4.5f),
            new AvoidLocation(40.5f, -3.5f),
            new AvoidLocation(40.5f, -2.5f),
            new AvoidLocation(41.5f, -16.5f),
            new AvoidLocation(41.5f, -15.5f),
            new AvoidLocation(41.5f, -14.5f),
            new AvoidLocation(41.5f, -13.5f),
            new AvoidLocation(41.5f, -12.5f),
            new AvoidLocation(41.5f, -11.5f),
            new AvoidLocation(41.5f, -10.5f),
            new AvoidLocation(41.5f, -9.5f),
            new AvoidLocation(41.5f, -8.5f),
            new AvoidLocation(41.5f, -7.5f),
            new AvoidLocation(41.5f, -6.5f),
            new AvoidLocation(41.5f, -5.5f),
            new AvoidLocation(41.5f, -4.5f),
            new AvoidLocation(41.5f, -3.5f),
            new AvoidLocation(41.5f, -2.5f),
            new AvoidLocation(42.5f, -16.5f),
            new AvoidLocation(42.5f, -15.5f),
            new AvoidLocation(42.5f, -14.5f),
            new AvoidLocation(42.5f, -13.5f),
            new AvoidLocation(42.5f, -11.5f),
            new AvoidLocation(42.5f, -10.5f),
            new AvoidLocation(42.5f, -9.5f),
            new AvoidLocation(42.5f, -8.5f),
            new AvoidLocation(42.5f, -7.5f),
            new AvoidLocation(42.5f, -6.5f),
            new AvoidLocation(43.5f, -16.5f),
            new AvoidLocation(43.5f, -15.5f),
            new AvoidLocation(43.5f, -14.5f),
            new AvoidLocation(43.5f, -13.5f),
            new AvoidLocation(43.5f, -12.5f),
            new AvoidLocation(43.5f, -11.5f),
            new AvoidLocation(43.5f, -10.5f),
            new AvoidLocation(43.5f, -9.5f),
            new AvoidLocation(43.5f, -8.5f),
            new AvoidLocation(43.5f, -7.5f),
            new AvoidLocation(43.5f, -6.5f),
            new AvoidLocation(44.5f, -16.5f),
            new AvoidLocation(44.5f, -15.5f),
            new AvoidLocation(44.5f, -14.5f),
            new AvoidLocation(44.5f, -13.5f),
            new AvoidLocation(44.5f, -12.5f),
            new AvoidLocation(44.5f, -11.5f),
            new AvoidLocation(44.5f, -10.5f),
            new AvoidLocation(44.5f, -9.5f),
            new AvoidLocation(44.5f, -8.5f),
            new AvoidLocation(44.5f, -7.5f),
            new AvoidLocation(44.5f, -6.5f),
            new AvoidLocation(44.5f, -5.5f),
            new AvoidLocation(45.5f, -16.5f),
            new AvoidLocation(45.5f, -15.5f),
            new AvoidLocation(45.5f, -14.5f),
            new AvoidLocation(45.5f, -13.5f),
            new AvoidLocation(45.5f, -12.5f),
            new AvoidLocation(45.5f, -11.5f),
            new AvoidLocation(45.5f, -10.5f),
            new AvoidLocation(45.5f, -9.5f),
            new AvoidLocation(45.5f, -8.5f),
            new AvoidLocation(45.5f, -7.5f),
            new AvoidLocation(45.5f, -6.5f),
            new AvoidLocation(45.5f, -5.5f),
            new AvoidLocation(45.5f, -4.5f),
            new AvoidLocation(46.5f, -16.5f),
            new AvoidLocation(46.5f, -15.5f),
            new AvoidLocation(46.5f, -14.5f),
            new AvoidLocation(46.5f, -13.5f),
            new AvoidLocation(46.5f, -12.5f),
            new AvoidLocation(46.5f, -11.5f),
            new AvoidLocation(46.5f, -10.5f),
            new AvoidLocation(46.5f, -9.5f),
            new AvoidLocation(46.5f, -8.5f),
            new AvoidLocation(46.5f, -7.5f),
            new AvoidLocation(46.5f, -6.5f),
            new AvoidLocation(46.5f, -5.5f),
            new AvoidLocation(46.5f, -4.5f),
            new AvoidLocation(46.5f, -3.5f),
            new AvoidLocation(46.5f, -0.5f),
        };

        // Act
        var result = Planner.Execute(options, blueprint, avoid);

        // Assert
#if USE_VERIFY
        await Verify(GetGridString(result));
#else
        await Task.Yield();
#endif
    }

    /// <summary>
    /// This blueprint found a bug in the SortedBatches class.
    /// </summary>
    [Fact]
    public void PlansElectricPoles()
    {
        // Arrange
        var options = OilFieldOptions.ForSubstation;
        options.ValidateSolution = true;
        var blueprintString = "0eJyM00sOgyAQANC7zJqFgp+WqzRN42fS0CoSwabGePeidNHEJsySYXjM8Fmg7iY0o9IO5AKqGbQFeVnAqruuui2mqx5Bgpl686iaJzBws9kiymEPKwOlW3yDTNcrA9ROOYXB2AfzTU99jaNPYH8sM1i/YNDbTh4RGYPZpwrvtmrEJswlKztwnMDxPHBZnBMELi0CV8S5jFJdErgyzuWU6sTO8TTOFQQuS8hcSWk2nB0n3OyJ0my4WZ7HuTO9WXGozr/p/Z3Ln4/C4IWj/SasHwAAAP//AwCxgxNI";
        var blueprint = ParseBlueprint.Execute(blueprintString);

        // Act
        var (context, _) = Planner.Execute(options, blueprint);

        // Assert
        Assert.NotEmpty(context.Grid.GetEntities().OfType<ElectricPoleCenter>());
        Assert.NotEmpty(context.Grid.GetEntities().OfType<ElectricPoleSide>());
    }

    /// <summary>
    /// https://github.com/teoxoy/factorio-blueprint-editor/issues/253
    /// </summary>
    [Fact]
    public void FbeOriginalFallsBackToFbeWhenLeftoverPumpsCannotConnect()
    {
        // Arrange
        var options = OilFieldOptions.ForMediumElectricPole;
        options.ValidateSolution = true;
        options.PipeStrategies = new List<PipeStrategy> { PipeStrategy.FbeOriginal };
        // Big-list index 827. The blueprint that used to sit here stopped reaching this fallback once the
        // Factorio 2.1 terminal offsets moved the east and west pipe corners, so it was replaced with a field
        // that still does. Found by making this branch throw and scanning both corpus lists for the hit.
        var blueprintString = "0eJyU1MFuwyAMANB/8ZlDIEADvzJNU5qiia4hKCHTooh/H4HLqrbCOwLmAcL2DufbavxsXQC9gx0mt4B+22Gxn66/HXOuHw1o8Ovor/3wBQTC5o8ZG8wIkYB1F/MDmsZ3AsYFG6wpRh5sH24dz2ZOAeSJ5aclbZjccVJCGCOwge4Se7GzGcpSE8mDxhAaFVlTda3FaAqrcYx2yhpl9xx/wgkMxwvX1m8nEVx5KuV17YT51RbNdRiuKZyscwrBdUVD5Bxt/vGxGA9VEuW1rEF4qKKQxaP1xKOYsiipwhCJR1F10b7wUnfJHUf/aVkEvs285AAhmeJKCSFoxyWP8RcAAP//AwC8p5dm";

        var blueprint = ParseBlueprint.Execute(blueprintString);

        // Act
        var (_, summary) = Planner.Execute(options, blueprint);

        // Assert
        var plan = Assert.Single(summary.SelectedPlans);
        Assert.Equal(PipeStrategy.Fbe, plan.PipeStrategy);
    }

    /// <summary>
    /// https://github.com/teoxoy/factorio-blueprint-editor/issues/254
    /// </summary>
    [Fact]
    public void FbeOriginalFallsBackToFbeWhenAloneGroupRemains()
    {
        // Arrange
        var options = OilFieldOptions.ForMediumElectricPole;
        options.ValidateSolution = true;
        options.PipeStrategies = new List<PipeStrategy> { PipeStrategy.FbeOriginal };
        var blueprintString = "0eNqVlsluhDAQRP+lzxywu22WX4miaBYrcjJ4EEuU0Yh/D4M5RBNHFEeMebTbVWXf6XgZXdv5MFB9J3+6hp7qlzv1/j0cLo+xcGgc1dSOTftxOH1SRsOtfYz4wTU0ZeTD2X1TrabXjFwY/OBdZCwPt7cwNkfXzROyBKu99vMH1/D40wzhMqPbPJVn7tl37hTf5VP2B6cBnMkjzmzjGKnORly5jRMAZ9fqqm2cAXCiFpzOt3EW6Z2OOLWNKxBcEXF6G1ciW2EiDhBKhfSugHEqR8qrIk8AHuILxZEHCFkhxtBrfRbgIc4Qk+RJiodYg9f1FkB9kDdynLfHHAgPcQev+gOSRSH2kLV/FbAfkD/KJC+Zyzv8wUC4aIXHAQPpojWeB8+8VP80dHJE/TFyriH+EI3zDH5QQjy7Y3+f8q9M8Yod67UAr8TzioH804g/WCIPyAPOd+gFyANWO/QM+Jc1ns8C+JcZ14sA+mPB80CQ9ULnh/zDm++8yz24/nWRzujLdf0ywVhdSVUZY1QpVqbpB1ASvGc=";

        var blueprint = ParseBlueprint.Execute(blueprintString);

        // Act
        var (_, summary) = Planner.Execute(options, blueprint);

        // Assert
        var plan = Assert.Single(summary.SelectedPlans);
        Assert.Equal(PipeStrategy.Fbe, plan.PipeStrategy);
    }
}
