//! Economy subsystem: market simulation, faction economies, and production chains.

pub mod market;
pub mod factions;
pub mod production;

pub use market::{
    Market, CommodityId, OrderId, AuctionId,
    Order, OrderSide, TradeRecord, PricePoint,
    AuctionType, AuctionState, Auction,
    ManipulationAlert, ArbitrageOpportunity,
};

pub use factions::{
    FactionEconomy, FactionId, TradeRouteId, EmbargoPenalty,
    FactionTreasury, TaxPolicy, TradeRoute, TradeRouteStatus,
    WealthRanking, EspionageReport, TributeAgreement,
};

pub use production::{
    ProductionManager, NodeId, BuildingId, WorkerId,
    ResourceNode, NodeKind, ProcessingBuilding, Recipe,
    ProductionQuota, SupplyChainEvent, Stockpile,
    EfficiencyModifier, ProductionReport,
};
