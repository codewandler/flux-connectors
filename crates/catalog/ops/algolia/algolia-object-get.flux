op algolia-object-get(index_name: String, object_id: String) -> Any
  description "Read one record from an index by its object id. Returns the stored record as it is; its attributes are this index's own content model, so nothing here can name them. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/message` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{app_id}.algolia.net"
  X_Algolia_Application_Id = "{app_id}"
  url = fmt("{base}/1/indexes/{index_name}/{object_id}")
  response = http.request(headers: { "X-Algolia-Application-Id": X_Algolia_Application_Id }, method: "GET", url)
  return response
