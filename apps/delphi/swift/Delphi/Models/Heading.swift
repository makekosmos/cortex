import Foundation
import SwiftData

@Model
final class Heading {
    var id: UUID
    var title: String
    var sortOrder: Int

    @Relationship
    var project: Project?

    init(title: String, project: Project? = nil) {
        self.id = UUID()
        self.title = title
        self.sortOrder = 0
        self.project = project
    }
}
