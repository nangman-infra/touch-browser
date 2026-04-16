export type GenerateMcpToolCatalogResult = {
  schema: string;
  generatedJson: string;
  generatedModule: string;
  toolCount: number;
};

export function generateMcpToolCatalog(
  root: string,
): Promise<GenerateMcpToolCatalogResult>;
