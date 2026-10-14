(ns edge-cases
  "A namespace demonstrating edge cases for clj-kondo extraction.")

(defn- private-fn
  [x]
  (inc x))

(defn public-fn
  "This is a public function docstring."
  [y]
  (let [result (private-fn y)]
    (println "Result:" result)
    result))

(defn threaded-calls
  [data]
  (-> data
      (private-fn)
      (str " processed")))
