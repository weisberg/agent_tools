/// Inspection handlers — read-only queries about presentation structure.

export async function inspect(_params: any): Promise<any> {
  return PowerPoint.run(async (context) => {
    const slides = context.presentation.slides;
    slides.load('id');
    await context.sync();

    return {
      slideCount: slides.items.length,
    };
  });
}

export async function inspectMasters(_params: any): Promise<any> {
  return PowerPoint.run(async (context) => {
    const masters = context.presentation.slideMasters;
    masters.load('id, name');
    await context.sync();

    for (const master of masters.items) {
      master.layouts.load('id, name');
    }
    await context.sync();

    const schema = masters.items.map((master) => ({
      id: master.id,
      name: master.name,
      layouts: master.layouts.items.map((l) => ({
        id: l.id,
        name: l.name,
      })),
    }));

    return { masters: schema };
  });
}

export async function inspectTheme(_params: any): Promise<any> {
  // TODO: PowerPoint.js theme API is limited.
  // This is a placeholder that returns what's available.
  return PowerPoint.run(async (context) => {
    const masters = context.presentation.slideMasters;
    masters.load('id, name');
    await context.sync();

    return {
      message: 'Theme inspection — expand as Office.js API coverage grows',
      masterCount: masters.items.length,
    };
  });
}
