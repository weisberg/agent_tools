/// Write handlers — modify shape properties.

export async function setText(params: {
  slideIndex: number;
  shapeId: string;
  text: string;
}): Promise<any> {
  return PowerPoint.run(async (context) => {
    const slide = context.presentation.slides.getItemAt(params.slideIndex);
    const shape = slide.shapes.getItem(params.shapeId);
    shape.textFrame.textRange.text = params.text;
    shape.load('id, name');
    await context.sync();

    return { shapeId: shape.id, name: shape.name, text: params.text };
  });
}

export async function setFill(params: {
  slideIndex: number;
  shapeId: string;
  color: string;
}): Promise<any> {
  return PowerPoint.run(async (context) => {
    const slide = context.presentation.slides.getItemAt(params.slideIndex);
    const shape = slide.shapes.getItem(params.shapeId);
    shape.fill.setSolidColor(params.color);
    shape.load('id, name');
    await context.sync();

    return { shapeId: shape.id, name: shape.name, fill: params.color };
  });
}

export async function setFont(params: {
  slideIndex: number;
  shapeId: string;
  size?: number;
  bold?: boolean;
  italic?: boolean;
}): Promise<any> {
  return PowerPoint.run(async (context) => {
    const slide = context.presentation.slides.getItemAt(params.slideIndex);
    const shape = slide.shapes.getItem(params.shapeId);
    const font = shape.textFrame.textRange.font;

    if (params.size !== undefined) font.size = params.size;
    if (params.bold !== undefined) font.bold = params.bold;
    if (params.italic !== undefined) font.italic = params.italic;

    shape.load('id, name');
    await context.sync();

    return {
      shapeId: shape.id,
      name: shape.name,
      font: { size: params.size, bold: params.bold, italic: params.italic },
    };
  });
}

export async function setGeometry(params: {
  slideIndex: number;
  shapeId: string;
  left?: number;
  top?: number;
  width?: number;
  height?: number;
}): Promise<any> {
  return PowerPoint.run(async (context) => {
    const slide = context.presentation.slides.getItemAt(params.slideIndex);
    const shape = slide.shapes.getItem(params.shapeId);

    if (params.left !== undefined) shape.left = params.left;
    if (params.top !== undefined) shape.top = params.top;
    if (params.width !== undefined) shape.width = params.width;
    if (params.height !== undefined) shape.height = params.height;

    shape.load('id, name, left, top, width, height');
    await context.sync();

    return {
      shapeId: shape.id,
      name: shape.name,
      geometry: {
        left: shape.left,
        top: shape.top,
        width: shape.width,
        height: shape.height,
      },
    };
  });
}
