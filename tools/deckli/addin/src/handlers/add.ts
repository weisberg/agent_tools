/// Add handlers — create new slides, shapes, images, tables.

export async function addSlide(params: {
  layoutName: string;
  position?: number;
}): Promise<any> {
  return PowerPoint.run(async (context) => {
    // Find the layout by name across all masters
    const masters = context.presentation.slideMasters;
    masters.load('id');
    await context.sync();

    let targetLayout: PowerPoint.SlideLayout | null = null;

    for (const master of masters.items) {
      master.layouts.load('id, name');
    }
    await context.sync();

    for (const master of masters.items) {
      for (const layout of master.layouts.items) {
        if (layout.name === params.layoutName) {
          targetLayout = layout;
          break;
        }
      }
      if (targetLayout) break;
    }

    if (!targetLayout) {
      const available = masters.items.flatMap((m) =>
        m.layouts.items.map((l) => l.name)
      );
      throw {
        code: 'layout_not_found',
        message: `Layout "${params.layoutName}" not found`,
        suggestion: `Available layouts: ${available.join(', ')}. Run \`deckli inspect --masters\` for full list.`,
      };
    }

    const options: PowerPoint.AddSlideOptions = {
      slideMasterId: targetLayout.id,
      layoutId: targetLayout.id,
    };

    context.presentation.slides.add(options);
    await context.sync();

    // TODO: If position specified, move slide to that index
    return { layout: params.layoutName, position: params.position ?? 'end' };
  });
}

export async function addShape(params: {
  slideIndex: number;
  type: string;
  left: number;
  top: number;
  width: number;
  height: number;
  fill?: string;
  text?: string;
}): Promise<any> {
  return PowerPoint.run(async (context) => {
    const slide = context.presentation.slides.getItemAt(params.slideIndex);

    const shapeType =
      (PowerPoint.GeometricShapeType as any)[params.type] ||
      PowerPoint.GeometricShapeType.rectangle;

    const shape = slide.shapes.addGeometricShape(shapeType, {
      left: params.left,
      top: params.top,
      width: params.width,
      height: params.height,
    });

    shape.load('id, name');

    if (params.fill) {
      shape.fill.setSolidColor(params.fill);
    }
    if (params.text) {
      shape.textFrame.textRange.text = params.text;
    }

    await context.sync();

    return {
      shapeId: shape.id,
      name: shape.name,
      type: params.type,
      geometry: {
        left: params.left,
        top: params.top,
        width: params.width,
        height: params.height,
      },
    };
  });
}

export async function addImage(params: {
  slideIndex: number;
  imageBase64: string;
  format: string;
  left: number;
  top: number;
  width: number;
  height: number;
}): Promise<any> {
  return PowerPoint.run(async (context) => {
    const slide = context.presentation.slides.getItemAt(params.slideIndex);

    const image = slide.shapes.addImage(params.imageBase64, {
      left: params.left,
      top: params.top,
      width: params.width,
      height: params.height,
    });

    image.load('id, name');
    await context.sync();

    return {
      shapeId: image.id,
      name: image.name,
      format: params.format,
      geometry: {
        left: params.left,
        top: params.top,
        width: params.width,
        height: params.height,
      },
    };
  });
}

export async function addTable(params: {
  slideIndex: number;
  data: string[][];
  left: number;
  top: number;
  width: number;
  height: number;
}): Promise<any> {
  // TODO: PowerPoint.js table API — may need to build from shapes
  // For now, create a textbox with formatted table text
  return PowerPoint.run(async (context) => {
    const slide = context.presentation.slides.getItemAt(params.slideIndex);

    const rows = params.data.length;
    const cols = params.data[0]?.length ?? 0;

    // Placeholder: add as textbox with tab-separated content
    const text = params.data.map((row) => row.join('\t')).join('\n');

    const shape = slide.shapes.addGeometricShape(
      PowerPoint.GeometricShapeType.rectangle,
      {
        left: params.left,
        top: params.top,
        width: params.width,
        height: params.height,
      },
    );

    shape.textFrame.textRange.text = text;
    shape.load('id, name');
    await context.sync();

    return {
      shapeId: shape.id,
      name: shape.name,
      rows,
      cols,
      note: 'Table rendered as textbox — native table API pending',
    };
  });
}
