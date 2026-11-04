// Kept apart from memoryGraph.ts so the pages that only name a layout do not
// load the graph libraries.

export type LayoutName = 'vertical' | 'horizontal' | 'organic'

export const LAYOUTS: Record<LayoutName, string> = {
  vertical: 'Hiérarchie verticale',
  horizontal: 'Hiérarchie horizontale',
  organic: 'Organique',
}
