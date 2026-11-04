// Kept apart from memoryGraph.ts so the pages that only name a layout do not
// load the graph libraries.

import { labels } from './i18n.svelte'

export type LayoutName = 'vertical' | 'horizontal' | 'organic'

export const LAYOUTS = labels<LayoutName>({
  vertical: ['Vertical hierarchy', 'Hiérarchie verticale'],
  horizontal: ['Horizontal hierarchy', 'Hiérarchie horizontale'],
  organic: ['Organic', 'Organique'],
})
