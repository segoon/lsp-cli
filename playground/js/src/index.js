import { buildSampleOrder } from "./order.js";
import { formatOrder } from "./report/formatter.js";

const order = buildSampleOrder();
console.log(formatOrder(order));
