/// Remove handlers — delete slides and shapes.

export async function removeSlide(params: {
  slideIndex: number;
}): Promise<any> {
  return PowerPoint.run(async (context) => {
    const slide = context.presentation.slides.getItemAt(params.slideIndex);
    slide.load('id');
    await context.sync();

    const id = slide.id;
    slide.delete();
    await context.sync();

    return { deleted: 'slide', slideIndex: params.slideIndex, id };
  });
}

export async function removeShape(params: {
  slideIndex: number;
  shapeId: string;
}): Promise<any> {
  return PowerPoint.run(async (context) => {
    const slide = context.presentation.slides.getItemAt(params.slideIndex);
    const shape = slide.shapes.getItem(params.shapeId);
    shape.load('id, name');
    await context.sync();

    const info = { id: shape.id, name: shape.name };
    shape.delete();
    await context.sync();

    return { deleted: 'shape', ...info };
  });
}
