(ns main)

(defrecord OrderItem [name quantity price])

(defrecord Order [customer items])

(defprotocol OrderFormatting
  (format-order-value [formatter order]))

(defn order-total [order]
  (reduce + (map (fn [item] (* (:quantity item) (:price item))) (:items order))))

(defn build-sample-order []
  (->Order "Carol" [(->OrderItem "Mouse" 1 35.0) (->OrderItem "Pad" 1 12.5)]))

(defrecord PlainOrderFormatter []
  OrderFormatting
  (format-order-value [_ order]
    (str (:customer order) " has " (count (:items order)) " items worth " (order-total order))))

(def ^PlainOrderFormatter formatter (->PlainOrderFormatter))

(defn format-order [order]
  (format-order-value formatter order))

(println (format-order (build-sample-order)))
