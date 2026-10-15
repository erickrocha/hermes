import { resourceConfigs, vehicles, serviceTypes, pricedServices, parts, vehicleExpenses, customers, holidays, transportDemands, extraTrips, garageServices, purchaseOrders, checklistTemplates, preventivePlans, workOrders, fuelEntries, inspectionModels, maintenanceWindows, technicalInspections, checklistRuns, garageAttendances, fuelGaugeSettings, ruleSettings, internalTank } from './resources'

// Toda slice de feature passa por aqui para entrar no store.
export const allResources = [vehicles, serviceTypes, pricedServices, parts, vehicleExpenses, customers, holidays, transportDemands, extraTrips, garageServices, purchaseOrders, checklistTemplates, preventivePlans, workOrders, fuelEntries, inspectionModels, maintenanceWindows, technicalInspections, checklistRuns, garageAttendances, fuelGaugeSettings, ruleSettings, internalTank]
export { resourceConfigs }
