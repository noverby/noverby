import { resolve } from 'gql';

const toId = async (path: string[], parentId?: string): Promise<string | undefined> => {
  const where = {
    _and: [
      { key: { _eq: path.at(-1) } },
      parentId
        ? { parentId: { _eq: parentId } }
        : { parentId: { _is_null: true } },
    ],
  };
  const id = await resolve(({ query }) => query.nodes({ where }).at(0)?.id);

  return path.length > 0 ? toId(path.slice(1), id) : id;
};

const fromId = async (id?: string | null): Promise<string[]> => {
  if (!id) return [];
  return await resolve(async ({ query }) => {
    const node = query.node({ id });
    const parentId = node?.parentId;
    const key = node?.key;
    if (!parentId) return [];
    return (await fromId(parentId)).concat(key ? [key] : []);
  });
};

export { toId, fromId };
