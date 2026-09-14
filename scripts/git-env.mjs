const repositoryContext = new Set(
  "GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_ALTERNATE_OBJECT_DIRECTORIES GIT_CEILING_DIRECTORIES GIT_PREFIX".split(
    " ",
  ),
);
export const gitEnv = (env = process.env) =>
  Object.fromEntries(Object.entries(env).filter(([key]) => !repositoryContext.has(key)));
