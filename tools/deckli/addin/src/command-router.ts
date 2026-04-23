/// Routes incoming commands to the appropriate Office.js handler.
import * as handlers from './handlers';

export async function executeCommand(method: string, params: any): Promise<any> {
  switch (method) {
    // Inspection
    case 'inspect':           return handlers.inspect(params);
    case 'inspect.masters':   return handlers.inspectMasters(params);
    case 'inspect.theme':     return handlers.inspectTheme(params);

    // Read
    case 'get.slides':        return handlers.getSlides(params);
    case 'get.slide':         return handlers.getSlide(params);
    case 'get.shapes':        return handlers.getShapes(params);
    case 'get.shape':         return handlers.getShape(params);
    case 'get.notes':         return handlers.getNotes(params);
    case 'get.selection':     return handlers.getSelection(params);

    // Write
    case 'set.text':          return handlers.setText(params);
    case 'set.fill':          return handlers.setFill(params);
    case 'set.font':          return handlers.setFont(params);
    case 'set.geometry':      return handlers.setGeometry(params);

    // Add
    case 'add.slide':         return handlers.addSlide(params);
    case 'add.shape':         return handlers.addShape(params);
    case 'add.image':         return handlers.addImage(params);
    case 'add.table':         return handlers.addTable(params);

    // Remove
    case 'rm.slide':          return handlers.removeSlide(params);
    case 'rm.shape':          return handlers.removeShape(params);

    // Reorder
    case 'move.slide':        return handlers.moveSlide(params);

    // Render
    case 'render.slide':      return handlers.renderSlide(params);

    // Batch
    case 'batch':             return handlers.executeBatch(params, executeCommand);

    default:
      throw {
        code: 'unknown_command',
        message: `Unknown command: ${method}`,
        suggestion: 'Run `deckli --help` for available commands',
      };
  }
}
