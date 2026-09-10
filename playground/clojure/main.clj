(ns main)

(defrecord OrderItem [name quantity price])

(defrecord Order [customer items])

(defn order-total [order]
  (reduce + (map (fn [item] (* (:quantity item) (:price item))) (:items order))))

(defn build-sample-order []
  (->Order "Carol" [(->OrderItem "Mouse" 1 35.0) (->OrderItem "Pad" 1 12.5)]))

(defn format-order [order]
  (str (:customer order) " has " (count (:items order)) " items worth " (order-total order)))

(println (format-order (build-sample-order)))
