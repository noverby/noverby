import React from 'react';
import { Button } from '@mui/material';
import { useRefetch } from 'gql';
import { AdminCard } from 'comps';
import { Node } from 'hooks';
import { Stop } from '@mui/icons-material';

const PollAdmin = ({ node }: { node: Node }) => {
  const sub = node.useSubs();
  const update = node.useUpdate();
  const data = sub?.data();
  const refetch = useRefetch();

  const voters = sub?.context
    ?.permissions({
      where: {
        _and: [
          { mimeId: { _eq: 'vote/vote' } },
          { insert: { _eq: true } },
          {
            node: {
              members: { active: { _eq: true } },
            },
          },
        ],
      },
    })
    .map((perm) => perm.node?.members_aggregate().aggregate?.count())
    .reduce((total, next) => (total ?? 0) + (next ?? 0), 0);

  const handleStopPoll = () => {
    update({ set: { mutable: false, data: { ...data, voters } } });
    //await refetch(() =>
    //  node.query
    //    ?.children({ where: { mimeId: { _eq: "vote/vote" } } })
    //    .map((vote) => vote.data)
    //);
  };

  if (!sub?.mutable || !sub?.isContextOwner) return null;

  return (
    <AdminCard title="Administrer Afstemning">
      <Button
        size="large"
        color="secondary"
        variant="contained"
        endIcon={<Stop />}
        sx={{ color: '#fff', m: 2 }}
        onClick={handleStopPoll}
      >
        Stop
      </Button>
    </AdminCard>
  );
};

export default PollAdmin;
