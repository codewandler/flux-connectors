op box-folder-items-list(folder_id: String) -> Any
  description "List the files and folders directly inside a folder, first page only (Box's own default page size). Each entry is a mini item representation — its type, id and name — not the full metadata box-file-get or box-folder-get returns. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/message`, its error code at `/code` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.box.com"
  url = fmt("{base}/2.0/folders/{folder_id}/items")
  response = http.request(method: "GET", url)
  return response
