op clickup-space-folder-list(space_id: String, archived: Bool) -> Any
  description "List a space's folders. Each folder's own lists are nested inline in the response — read a folder's `lists` field for the list ids clickup-list-task-list and clickup-task-create take, rather than looking for a separate folder-to-list operation"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.clickup.com/api/v2"
  url = fmt("{base}/space/{space_id}/folder")
  response = http.request(method: "GET", query: { archived }, url)
  return response
