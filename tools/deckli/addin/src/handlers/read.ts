/// Read handlers — retrieve slide/shape content.

export async function getSlides(_params: any): Promise<any> {
  return PowerPoint.run(async (context) => {
    const slides = context.presentation.slides;
    slides.load('id');
    await context.sync();

    const result = [];
    for (let i = 0; i < slides.items.length; i++) {
      const slide = slides.items[i];
      slide.shapes.load('id, name, type');
      result.push({ index: i, id: slide.id });
    }
    await context.sync();

    for (let i = 0; i < slides.items.length; i++) {
      (result[i] as any).shapeCount = slides.items[i].shapes.items.length;
    }

    return { slides: result };
  });
}

export async function getSlide(params: { slideIndex: number }): Promise<any> {
  return PowerPoint.run(async (context) => {
    const slide = context.presentation.slides.getItemAt(params.slideIndex);
    slide.load('id');
    slide.shapes.load('id, name, type, left, top, width, height');
    await context.sync();

    const shapes = slide.shapes.items.map((s) => ({
      id: s.id,
      name: s.name,
      type: s.type,
      geometry: {
        left: s.left,
        top: s.top,
        width: s.width,
        height: s.height,
      },
    }));

    return {
      slideIndex: params.slideIndex,
      id: slide.id,
      shapes,
    };
  });
}

export async function getShapes(params: { slideIndex: number }): Promise<any> {
  return getSlide(params); // Same data
}

export async function getShape(params: {
  slideIndex: number;
  shapeId: string;
}): Promise<any> {
  return PowerPoint.run(async (context) => {
    const slide = context.presentation.slides.getItemAt(params.slideIndex);
    const shape = slide.shapes.getItem(params.shapeId);
    shape.load('id, name, type, left, top, width, height');
    shape.textFrame.load('hasText');
    await context.sync();

    const result: any = {
      id: shape.id,
      name: shape.name,
      type: shape.type,
      geometry: {
        left: shape.left,
        top: shape.top,
        width: shape.width,
        height: shape.height,
      },
    };

    if (shape.textFrame.hasText) {
      shape.textFrame.textRange.load('text');
      await context.sync();
      result.text = shape.textFrame.textRange.text;
    }

    return result;
  });
}

export async function getNotes(params: { slideIndex: number }): Promise<any> {
  // TODO: Notes API may have limited support
  return PowerPoint.run(async (context) => {
    const slide = context.presentation.slides.getItemAt(params.slideIndex);
    slide.load('id');
    await context.sync();

    return {
      slideIndex: params.slideIndex,
      notes: '(notes reading not yet implemented — Office.js API limitation)',
    };
  });
}

export async function getSelection(_params: any): Promise<any> {
  // TODO: Selection API — depends on PowerPointApi version
  return {
    message: 'Selection reading not yet implemented — requires PowerPointApi 1.5+',
  };
}
