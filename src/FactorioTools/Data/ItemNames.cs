namespace Knapcode.FactorioTools.Data;

public static class ItemNames
{
    public static class Vanilla
    {
        // Renamed in Factorio 2.0. "effectivity-module-3" no longer exists, so the game
        // silently rejects it. See base/migrations/2.0.0.json in the game data.
        public const string EfficiencyModule3 = "efficiency-module-3";
        public const string ProductivityModule3 = "productivity-module-3";
        public const string SpeedModule3 = "speed-module-3";

        public const string Blueprint = "blueprint";
    }
}
