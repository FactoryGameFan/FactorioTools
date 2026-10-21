#if ENABLE_VISUALIZER
using System.Diagnostics;
using System.Globalization;
using System.Text;
using DelaunatorSharp;
using System;
using System.Collections.Generic;

namespace Knapcode.FactorioTools.OilField;

/// <summary>
/// Debug-only drawing of a grid. Writes an SVG by hand so the core project needs no image library.
/// </summary>
public static class Visualizer
{
    private const int CellSize = 64;

    public static void Show(SquareGrid grid)
    {
        Show(grid, Array.Empty<IPoint>(), Array.Empty<IEdge>());
    }

    public static void Show(SquareGrid grid, IReadOnlyCollection<IPoint> points, IReadOnlyCollection<IEdge> edges)
    {
        var path = System.IO.Path.Combine(System.IO.Path.GetTempPath(), "Knapcode.FactorioTools.OilField.svg");
        Save(grid, path, points, edges);

        var p = new Process();
        p.StartInfo = new ProcessStartInfo(path) { UseShellExecute = true };
        p.Start();
    }

    private static void Save(SquareGrid grid, string path, IReadOnlyCollection<IPoint> points, IReadOnlyCollection<IEdge> edges)
    {
        var width = grid.Width * CellSize;
        var height = grid.Height * CellSize;

        var svg = new StringBuilder();
        svg.Append(Invariant($"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">\n"));

        for (var y = 0; y < grid.Height; y++)
        {
            for (var x = 0; x < grid.Width; x++)
            {
                var location = new Location(x, y);
                string color;
                GridEntity? entity;
                if ((entity = grid[location]) is not null)
                {
                    color = entity switch
                    {
                        PumpjackSide _ => "green",
                        PumpjackCenter _ => "darkgreen",
                        Terminal _ => "darkgray",
                        Pipe _ => "gray",
                        ElectricPoleSide _ => "yellow",
                        ElectricPoleCenter _ => "gold",
                        BeaconSide _ => "mediumpurple",
                        BeaconCenter _ => "purple",
                        TemporaryEntity _ => "pink",
                        _ => throw new NotImplementedException(),
                    };
                }
                else
                {
                    color = "white";
                }

                const int gridLineWidth = 1;
                AppendRect(svg, location.X * CellSize, location.Y * CellSize, CellSize, CellSize, "gray");
                AppendRect(svg, location.X * CellSize + gridLineWidth, location.Y * CellSize + gridLineWidth, CellSize - 2 * gridLineWidth, CellSize - 2 * gridLineWidth, color);
            }
        }

        var pairs = new HashSet<(Location A, Location B)>();
        foreach (var location in grid.EntityLocations.EnumerateItems())
        {
            var center = grid[location] as ElectricPoleCenter;
            if (center is null)
            {
                continue;
            }

            foreach (var neighbor in center.Neighbors)
            {
                var (a, b) = (location, grid.EntityIdToLocation[neighbor]);
                if (a > b)
                {
                    (a, b) = (b, a);
                }

                if (pairs.Add((a, b)))
                {
                    AppendLine(svg, a.X * CellSize + CellSize / 2, a.Y * CellSize + CellSize / 2, b.X * CellSize + CellSize / 2, b.Y * CellSize + CellSize / 2, "chocolate", thickness: 4);
                }
            }
        }

        foreach (var p in points)
        {
            svg.Append(Invariant($"<circle cx=\"{ConvertX(p)}\" cy=\"{ConvertY(p)}\" r=\"{CellSize / 4.0}\" fill=\"red\"/>\n"));
        }

        foreach (var e in edges)
        {
            AppendLine(svg, ConvertX(e.P), ConvertY(e.P), ConvertX(e.Q), ConvertY(e.Q), "red", thickness: 5);
        }

        // Draw the grid
        const double gridOpacity = 0.5;
        const int thickness = 4;

        AppendLine(svg, width - 1, 0, width - 1, height - 1, "red", thickness, gridOpacity);
        AppendLine(svg, width - 1, height - 1, 0, height - 1, "red", thickness, gridOpacity);

        for (var x = 0; x < width; x += 10 * CellSize)
        {
            AppendLine(svg, x, 0, x, height - 1, "red", thickness, gridOpacity);
        }

        for (var y = 0; y < height; y += 10 * CellSize)
        {
            AppendLine(svg, width - 1, y, 0, y, "red", thickness, gridOpacity);
        }

        svg.Append("</svg>\n");
        System.IO.File.WriteAllText(path, svg.ToString());
    }

    private static void AppendRect(StringBuilder svg, int x, int y, int width, int height, string color)
    {
        svg.Append(Invariant($"<rect x=\"{x}\" y=\"{y}\" width=\"{width}\" height=\"{height}\" fill=\"{color}\"/>\n"));
    }

    private static void AppendLine(StringBuilder svg, double x1, double y1, double x2, double y2, string color, int thickness, double opacity = 1)
    {
        svg.Append(Invariant($"<line x1=\"{x1}\" y1=\"{y1}\" x2=\"{x2}\" y2=\"{y2}\" stroke=\"{color}\" stroke-width=\"{thickness}\" stroke-opacity=\"{opacity}\"/>\n"));
    }

    private static string Invariant(FormattableString value)
    {
        return value.ToString(CultureInfo.InvariantCulture);
    }

    private static double ConvertX(IPoint p)
    {
        return (p.X + 0.5) * CellSize;
    }

    private static double ConvertY(IPoint p)
    {
        return (p.Y + 0.5) * CellSize;
    }
}
#endif
