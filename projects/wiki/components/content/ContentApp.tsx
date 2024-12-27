import { Card } from '@mui/material';
import { Content, ContentHeader } from 'comps';
import { Node, useScreen } from 'hooks';
import { Suspense } from 'react';

const ContentApp = ({
  node,
  hideMembers,
  add,
}: {
  node: Node;
  hideMembers?: boolean;
  add?: boolean;
}) => {
  const screen = useScreen();
  return (
    <Card sx={{ m: 0 }}>
      <ContentHeader hideMembers={hideMembers} add={add} node={node} />
      <Suspense>
        <Content node={node} fontSize={screen ? '150%' : '100%'} />
      </Suspense>
    </Card>
  );
};

export default ContentApp;
