import { useQuery } from 'gql';
import { useSession } from 'hooks';
import { startTransition, useEffect } from 'react';
import { Loader } from 'comps';

const PathLoader = ({
  keys,
  parentId,
  fullpath,
}: {
  keys: string[];
  parentId?: string;
  fullpath: string[];
}) => {
  const query = useQuery();
  const [, setSession] = useSession();
  const where = {
    _and: [
      { key: { _eq: keys.at(-1) } },
      parentId
        ? { parentId: { _eq: parentId } }
        : { parentId: { _is_null: true } },
    ],
  };
  const node = query.nodes({ where }).at(0);
  const nodeId = node?.id;
  const name = node?.name;
  const selected =
    keys.length === fullpath.length &&
    keys.every((v, i) => v === fullpath[i]);
  useEffect(() => {
    if (selected) {
      // eslint-disable-next-line functional/immutable-data
      document.title = name ?? 'RadikalWiki';
      startTransition(() => {
        setSession({ nodeId: nodeId ?? null });
      });
    }
  }, [name, selected]);

  return selected ? (
    <Loader id={node?.id} />
  ) : (
    <PathLoader
      keys={[...keys, fullpath[keys.length]]}
      parentId={node?.id}
      fullpath={fullpath}
    />
  );
};

export default PathLoader;
