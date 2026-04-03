import SwiftUI

struct TagChip: View {
    let tag: Tag
    var compact: Bool = false

    var body: some View {
        Text(tag.title)
            .font(compact ? .caption2 : .caption)
            .padding(.horizontal, compact ? 4 : 6)
            .padding(.vertical, compact ? 1 : 2)
            .background(tagColor.opacity(0.15))
            .foregroundStyle(tagColor)
            .clipShape(RoundedRectangle(cornerRadius: 4))
    }

    private var tagColor: Color {
        switch tag.color {
        case "red": .red
        case "orange": .orange
        case "yellow": .yellow
        case "green": .green
        case "blue": .blue
        case "purple": .purple
        case "pink": .pink
        default: .blue
        }
    }
}
