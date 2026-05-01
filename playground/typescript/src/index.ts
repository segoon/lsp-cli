import { buildSampleOrder } from "./order";
import { formatOrder } from "./report/formatter";

const order = buildSampleOrder();
console.log(formatOrder(order));
