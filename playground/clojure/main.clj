(ns main)

(defrecord OrderItem [name quantity price])

(defrecord Order [customer items])

(defprotocol OrderFormatting
  (format-order-value [formatter order]))

(defn order-total [order]
  (reduce + (map (fn [item] (* (:quantity item) (:price item))) (:items order))))

(defn sample-order-items []
  [(->OrderItem "Mouse" 1 35.0) (->OrderItem "Pad" 1 12.5)])

(defn build-sample-order []
  (->Order "Carol" (sample-order-items)))

(defrecord PlainOrderFormatter []
  OrderFormatting
  (format-order-value [_ order]
    (str (:customer order) " has " (count (:items order)) " items worth " (order-total order))))

(def ^PlainOrderFormatter formatter (->PlainOrderFormatter))

(defn format-order [order]
  (format-order-value formatter order))

(defn build-order-report []
  (format-order (build-sample-order)))

(println (build-order-report))
