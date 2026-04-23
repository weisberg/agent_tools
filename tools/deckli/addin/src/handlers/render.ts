/// Render handler — export slide as base64 PNG for vision verification.

export async function renderSlide(params: {
  slideIndex: number;
}): Promise<any> {
  return PowerPoint.run(async (context) => {
    const slide = context.presentation.slides.getItemAt(params.slideIndex);
    const base64 = slide.exportAsBase64();
    await context.sync();

    return {
      slideIndex: params.slideIndex,
      image_base64: base64.value,
      format: 'png',
    };
  });
}
