import Foundation
import SwiftData

@Observable
@MainActor
final class SidebarViewModel {
    var selectedSmartList: SmartList? = .inbox
    var selectedProject: Project?
    var selectedArea: Area?

    var isShowingNewProject = false
    var newProjectTitle = ""

    var collapsedAreaIDs: Set<UUID> = []

    func selectSmartList(_ list: SmartList) {
        selectedSmartList = list
        selectedProject = nil
        selectedArea = nil
    }

    func selectProject(_ project: Project) {
        selectedSmartList = nil
        selectedProject = project
        selectedArea = nil
    }

    func selectArea(_ area: Area) {
        selectedSmartList = nil
        selectedProject = nil
        selectedArea = area
    }

    func toggleAreaCollapse(_ area: Area) {
        if collapsedAreaIDs.contains(area.id) {
            collapsedAreaIDs.remove(area.id)
        } else {
            collapsedAreaIDs.insert(area.id)
        }
    }

    func collapseAllAreas(_ areas: [Area]) {
        collapsedAreaIDs = Set(areas.map(\.id))
    }

    func expandAllAreas() {
        collapsedAreaIDs.removeAll()
    }

    func soloArea(_ area: Area, allAreas: [Area]) {
        collapsedAreaIDs = Set(allAreas.map(\.id))
        collapsedAreaIDs.remove(area.id)
    }

    func isAreaCollapsed(_ area: Area) -> Bool {
        collapsedAreaIDs.contains(area.id)
    }
}
