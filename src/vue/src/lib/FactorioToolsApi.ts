// The JSON shapes the Rust planner (rust/contract, through planner-wasm) reads and writes.
// Hand-maintained: this started as the client swagger-typescript-api generated from the
// C# WebApp's swagger.json, and was frozen when the site moved to the Rust planner.
// FactorioToolsApi.test.ts checks these types against the responses recorded in
// rust/contract/fixtures/contract.jsonl, so a change to the contract shows up there.

/**
 * A Factorio 2.0 quality tier. The integer value is the quality bonus level used to scale
 * quality-affected stats (the engine skips a hidden level 4, so legendary is level 5).
 */
export enum Quality {
  Normal = "Normal",
  Uncommon = "Uncommon",
  Rare = "Rare",
  Epic = "Epic",
  Legendary = "Legendary",
}

/** The strategy to use while planning pipes between pumpjacks. */
export enum PipeStrategy {
  FbeOriginal = "FbeOriginal",
  Fbe = "Fbe",
  ConnectedCentersDelaunay = "ConnectedCentersDelaunay",
  ConnectedCentersDelaunayMst = "ConnectedCentersDelaunayMst",
  ConnectedCentersFlute = "ConnectedCentersFlute",
}

/** The strategy to use when planning beacon placement around the pumpjacks. */
export enum BeaconStrategy {
  FbeOriginal = "FbeOriginal",
  Fbe = "Fbe",
  Snug = "Snug",
}

/** The properties needed to normalize a oil field blueprint. */
export interface OilFieldNormalizeRequest {
  /**
   * The input blueprint containing at least one pumpjack.
   * @minLength 1
   */
  blueprint: string
}

/** The properties needed to normalize a oil field blueprint. */
export interface OilFieldNormalizeRequestResponse {
  /**
   * The input blueprint containing at least one pumpjack.
   * @minLength 1
   */
  blueprint: string
}

/** The normalized oil field blueprint. */
export interface OilFieldNormalizeResponse {
  /** The original request provided, included expanded defaults. */
  request: OilFieldNormalizeRequestResponse
  /** The output normalized blueprint. */
  blueprint: string
}

/** A particular attempt oil field plan. */
export interface OilFieldPlan {
  /** The pipe strategy used to generate the plan. */
  pipeStrategy: PipeStrategy
  /** Whether or not the pipe optimized was used. */
  optimizePipes: boolean
  /** Which beacon strategy, if any, was used. */
  beaconStrategy?: BeaconStrategy | null
  /**
   * The number of effects the beacons provided to pumpjacks. Higher is better.
   * @format int32
   */
  beaconEffectCount: number
  /**
   * The number of beacons in the plan. For the same number of beacon effects, lower is better.
   * @format int32
   */
  beaconCount: number
  /**
   * The number of pipes in the plan. For the same number of beacon effects and beacons, lower is better. If underground pipes are used, this only counts the upwards and downwards connections for the underground stretches of pipes.
   * @format int32
   */
  pipeCount: number
  /**
   * The number of pipes before beacons or underground pipes are placed.
   * @format int32
   */
  pipeCountWithoutUnderground: number
}

/** The properties needed to generate an oil field plan. */
export interface OilFieldPlanRequest {
  /**
   * Whether or not underground pipes (pipe-to-ground) should be used.
   * @default true
   */
  useUndergroundPipes?: boolean
  /**
   * Whether or not to add beacons around the pumpjacks.
   * @default true
   */
  addBeacons?: boolean
  /**
   * Whether or not to use the pipe optimizer after each pipe strategy is executed. If set to true, the best solution
   * found will still be used, meaning if the unoptimized pipe plan performs better, it will be preferred over the
   * corresponding optimized pipe plan.
   * @default true
   */
  optimizePipes?: boolean
  /**
   * Whether or to allow beacon effects to overlap. For Factorio mods like Space Exploration, beacon effects cannot
   * overlap otherwise pumpjacks will break down with a beacon overload. For vanilla Factorio, this should be true.
   * @default true
   */
  overlapBeacons?: boolean
  /**
   * Whether or not to add electric poles around the pumpjacks and (optionally) beacons.
   * @default true
   */
  addElectricPoles?: boolean
  /**
   * Whether or not to route a heat pipe network adjacent to every pumpjack and pipe. This is required on Aquilo
   * (Factorio 2.0 / Space Age) where unheated entities freeze. When enabled, the output blueprint is emitted in the
   * Factorio 2.0 version format (2.0 directions and module item format). The planner only routes the heat pipe network
   * and leaves it for the user to connect a heat source (heating tower or reactor). Heat pipes and beacons
   * (see Knapcode.FactorioTools.OilField.OilFieldOptions.AddBeacons) can be enabled together: heat is the hard constraint, so the planner routes the
   * heat network first and then places beacons around it, choosing a pipe layout it can fully heat. Beacons are
   * best-effort and may be reduced (even to none) on tight fields where heat needs the contested tiles.
   * @default false
   */
  addHeatPipes?: boolean
  /**
   * The internal entity name for the heat pipe to use.
   * @default "heat-pipe"
   */
  heatPipeEntityName?: string
  /**
   * The pipe planning strategies to attempt.
   * @default ["Fbe","ConnectedCentersDelaunay","ConnectedCentersDelaunayMst","ConnectedCentersFlute"]
   */
  pipeStrategies?: PipeStrategy[]
  /**
   * The beacon planning strategies to attempt. This will have no affect if Knapcode.FactorioTools.OilField.OilFieldOptions.AddBeacons is false.
   * @default ["Fbe","Snug"]
   */
  beaconStrategies?: BeaconStrategy[]
  /**
   * The internal entity name for the electric pole to use.
   * @default "medium-electric-pole"
   */
  electricPoleEntityName?: string
  /**
   * The supply width (horizontal) for the electric pole. This is the width of the area that the electric pole will
   * provide power to.
   * @format int32
   * @default 7
   */
  electricPoleSupplyWidth?: number
  /**
   * The supply height (vertical) for the electric pole. This is the height of the area that the electric pole will
   * provide power to.
   * @format int32
   * @default 7
   */
  electricPoleSupplyHeight?: number
  /**
   * The wire reach for the electric pole. This is how far apart electric poles can be but still be connected.
   * @format double
   * @default 9
   */
  electricPoleWireReach?: number
  /**
   * The width of the electric pole entity.
   * @format int32
   * @default 1
   */
  electricPoleWidth?: number
  /**
   * The height of the electric pole entity.
   * @format int32
   * @default 1
   */
  electricPoleHeight?: number
  /**
   * The internal entity name for the beacon to use.
   * @default "beacon"
   */
  beaconEntityName?: string
  /**
   * The supply width (horizontal) for the beacon. This is the width of the area that the beacon will provide
   * module effects to.
   * @format int32
   * @default 9
   */
  beaconSupplyWidth?: number
  /**
   * The supply height (vertical) for the beacon. This is the height of the area that the beacon will provide
   * module effects to.
   * @format int32
   * @default 9
   */
  beaconSupplyHeight?: number
  /**
   * The width of the beacon entity.
   * @format int32
   * @default 3
   */
  beaconWidth?: number
  /**
   * The height of the beacon entity.
   * @format int32
   * @default 3
   */
  beaconHeight?: number
  /**
   * Whether or not additional validations should be perform on the blueprint correctness. In most cases this should
   * be false. If you see an invalid blueprint returned, try setting this to true and reporting a bug.
   * @default false
   */
  validateSolution?: boolean
  /**
   * The modules to add to the pumpjacks. The string key is the internal item name for the module. The value is the
   * count that kind of module to add to each pumpjack. There can be multiple module types provided.
   * @default {"productivity-module-3":2}
   */
  pumpjackModules?: Record<string, number>
  /**
   * The modules to add to the beacons. The string key is the internal item name for the module. The value is the
   * count that kind of module to add to each beacon. There can be multiple module types provided.
   * @default {"speed-module-3":2}
   */
  beaconModules?: Record<string, number>
  /**
   * The quality of the pumpjack entities in the output blueprint. Output-only; does not affect planning.
   * @default "Normal"
   */
  pumpjackQuality?: Quality
  /**
   * The quality of the beacon entities in the output blueprint. Output-only; does not affect planning.
   * @default "Normal"
   */
  beaconQuality?: Quality
  /**
   * The quality of the electric pole entities. Higher quality enlarges the supply area and wire reach
   * the planner uses (see Context), so this affects planning, not just output.
   * @default "Normal"
   */
  electricPoleQuality?: Quality
  /**
   * The quality of the modules inserted into pumpjacks. Output-only; does not affect planning.
   * @default "Normal"
   */
  pumpjackModuleQuality?: Quality
  /**
   * The quality of the modules inserted into beacons. Output-only; does not affect planning.
   * @default "Normal"
   */
  beaconModuleQuality?: Quality
  /**
   * The input blueprint containing at least one pumpjack.
   * @minLength 1
   * @example "0eJyMj70OwjAMhN/lZg8NbHkVhFB/rMrQuFGSIqoq707aMiCVgcWSz+fP5wXNMLEPogl2gbSjRtjLgii91sOqae0YFn5y/l63DxDS7FdFEjtkgmjHL1iTrwTWJEl4Z2zNfNPJNRyKgX6w/BjLwqjrpQI5E+ZSC7WTwO0+qTIdYKc/YKbaaOaAK0G38Pbre8KTQ/wY8hsAAP//AwAEfF3F"
   */
  blueprint: string
  /**
   * Whether or not to add a placeholder entity to the output grid so that the planning grid entity coordinates match
   * the entity coordinate when the output blueprint is pasted into Factorio Blueprint Editor (FBE). This helps with
   * debugging the planner.
   * @default false
   */
  addFbeOffset?: boolean
}

/** The properties needed to generate an oil field plan. */
export interface OilFieldPlanRequestResponse {
  /** Whether or not underground pipes (pipe-to-ground) should be used. */
  useUndergroundPipes: boolean
  /** Whether or not to add beacons around the pumpjacks. */
  addBeacons: boolean
  /**
   * Whether or not to use the pipe optimizer after each pipe strategy is executed. If set to true, the best solution
   * found will still be used, meaning if the unoptimized pipe plan performs better, it will be preferred over the
   * corresponding optimized pipe plan.
   */
  optimizePipes: boolean
  /**
   * Whether or to allow beacon effects to overlap. For Factorio mods like Space Exploration, beacon effects cannot
   * overlap otherwise pumpjacks will break down with a beacon overload. For vanilla Factorio, this should be true.
   */
  overlapBeacons: boolean
  /** Whether or not to add electric poles around the pumpjacks and (optionally) beacons. */
  addElectricPoles: boolean
  /**
   * Whether or not to route a heat pipe network adjacent to every pumpjack and pipe. This is required on Aquilo
   * (Factorio 2.0 / Space Age) where unheated entities freeze. When enabled, the output blueprint is emitted in the
   * Factorio 2.0 version format (2.0 directions and module item format). The planner only routes the heat pipe network
   * and leaves it for the user to connect a heat source (heating tower or reactor). Heat pipes and beacons
   * (see Knapcode.FactorioTools.OilField.OilFieldOptions.AddBeacons) can be enabled together: heat is the hard constraint, so the planner routes the
   * heat network first and then places beacons around it, choosing a pipe layout it can fully heat. Beacons are
   * best-effort and may be reduced (even to none) on tight fields where heat needs the contested tiles.
   */
  addHeatPipes: boolean
  /** The internal entity name for the heat pipe to use. */
  heatPipeEntityName: string
  /** The pipe planning strategies to attempt. */
  pipeStrategies: PipeStrategy[]
  /** The beacon planning strategies to attempt. This will have no affect if Knapcode.FactorioTools.OilField.OilFieldOptions.AddBeacons is false. */
  beaconStrategies: BeaconStrategy[]
  /** The internal entity name for the electric pole to use. */
  electricPoleEntityName: string
  /**
   * The supply width (horizontal) for the electric pole. This is the width of the area that the electric pole will
   * provide power to.
   * @format int32
   */
  electricPoleSupplyWidth: number
  /**
   * The supply height (vertical) for the electric pole. This is the height of the area that the electric pole will
   * provide power to.
   * @format int32
   */
  electricPoleSupplyHeight: number
  /**
   * The wire reach for the electric pole. This is how far apart electric poles can be but still be connected.
   * @format double
   */
  electricPoleWireReach: number
  /**
   * The width of the electric pole entity.
   * @format int32
   */
  electricPoleWidth: number
  /**
   * The height of the electric pole entity.
   * @format int32
   */
  electricPoleHeight: number
  /** The internal entity name for the beacon to use. */
  beaconEntityName: string
  /**
   * The supply width (horizontal) for the beacon. This is the width of the area that the beacon will provide
   * module effects to.
   * @format int32
   */
  beaconSupplyWidth: number
  /**
   * The supply height (vertical) for the beacon. This is the height of the area that the beacon will provide
   * module effects to.
   * @format int32
   */
  beaconSupplyHeight: number
  /**
   * The width of the beacon entity.
   * @format int32
   */
  beaconWidth: number
  /**
   * The height of the beacon entity.
   * @format int32
   */
  beaconHeight: number
  /**
   * Whether or not additional validations should be perform on the blueprint correctness. In most cases this should
   * be false. If you see an invalid blueprint returned, try setting this to true and reporting a bug.
   */
  validateSolution: boolean
  /**
   * The modules to add to the pumpjacks. The string key is the internal item name for the module. The value is the
   * count that kind of module to add to each pumpjack. There can be multiple module types provided.
   */
  pumpjackModules: Record<string, number>
  /**
   * The modules to add to the beacons. The string key is the internal item name for the module. The value is the
   * count that kind of module to add to each beacon. There can be multiple module types provided.
   */
  beaconModules: Record<string, number>
  /** The quality of the pumpjack entities in the output blueprint. Output-only; does not affect planning. */
  pumpjackQuality: Quality
  /** The quality of the beacon entities in the output blueprint. Output-only; does not affect planning. */
  beaconQuality: Quality
  /**
   * The quality of the electric pole entities. Higher quality enlarges the supply area and wire reach
   * the planner uses (see Context), so this affects planning, not just output.
   */
  electricPoleQuality: Quality
  /** The quality of the modules inserted into pumpjacks. Output-only; does not affect planning. */
  pumpjackModuleQuality: Quality
  /** The quality of the modules inserted into beacons. Output-only; does not affect planning. */
  beaconModuleQuality: Quality
  /**
   * The input blueprint containing at least one pumpjack.
   * @minLength 1
   */
  blueprint: string
  /**
   * Whether or not to add a placeholder entity to the output grid so that the planning grid entity coordinates match
   * the entity coordinate when the output blueprint is pasted into Factorio Blueprint Editor (FBE). This helps with
   * debugging the planner.
   */
  addFbeOffset: boolean
}

/** The resulting oil field plan. */
export interface OilFieldPlanResponse {
  /** The original request provided, included expanded defaults. */
  request: OilFieldPlanRequestResponse
  /** The output blueprint, containing the planned oil field. */
  blueprint: string
  /** A summary of different oil field plans attempted and their performance. */
  summary: OilFieldPlanSummary
}

/** A summary of the various oil field plans attempted. */
export interface OilFieldPlanSummary {
  /**
   * The number of pumpjacks removed to allow for electric poles. This must be zero.
   * @format int32
   */
  missingPumpjacks: number
  /**
   * The number of pumpjacks that were rotated from their original position.
   * @format int32
   */
  rotatedPumpjacks: number
  /**
   * Pumpjacks dropped so the rest of the field could be fully heated on Aquilo. Zero unless heat pipes are enabled.
   * @format int32
   */
  heatDroppedPumpjacks: number
  /**
   * Pumpjacks still left without an adjacent heat pipe in the final output (normally zero).
   * @format int32
   */
  unheatedPumpjacks: number
  /**
   * Pipe tiles still left without an adjacent heat pipe in the final output (normally zero).
   * @format int32
   */
  unheatedPipes: number
  /** The set of plans which exactly the same and determined to be the best. */
  selectedPlans: OilFieldPlan[]
  /** The set of plans which are equivalent to the selected plans by ranking but not exactly the same. */
  alternatePlans: OilFieldPlan[]
  /** The set of plans that were not the best and were discarded. */
  unusedPlans: OilFieldPlan[]
}
