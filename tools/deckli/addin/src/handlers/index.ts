/// Re-exports all Office.js command handlers.
export { inspect, inspectMasters, inspectTheme } from './inspect';
export { getSlides, getSlide, getShapes, getShape, getNotes, getSelection } from './read';
export { setText, setFill, setFont, setGeometry } from './write';
export { addSlide, addShape, addImage, addTable } from './add';
export { removeSlide, removeShape } from './remove';
export { moveSlide } from './move';
export { renderSlide } from './render';
export { executeBatch } from './batch';
