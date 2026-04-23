/// Move handler — reorder slides.

export async function moveSlide(params: {
  fromIndex: number;
  toIndex: number;
}): Promise<any> {
  return PowerPoint.run(async (context) => {
    const slide = context.presentation.slides.getItemAt(params.fromIndex);
    slide.load('id');
    await context.sync();

    // moveTo is available in PowerPointApi 1.2+
    (slide as any).moveTo(params.toIndex);
    await context.sync();

    return {
      moved: 'slide',
      from: params.fromIndex,
      to: params.toIndex,
      id: slide.id,
    };
  });
}
