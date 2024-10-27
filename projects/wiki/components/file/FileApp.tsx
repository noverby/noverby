import { Card, Collapse, Stack } from '@mui/material';
import { FileLoader, ChangeList, QuestionList, ContentHeader } from 'comps';
import { Node } from 'hooks';

const FileApp = ({ node }: { node: Node }) => {
  return (
    <>
      <Stack spacing={1}>
        <Card sx={{ m: 0 }}>
          <ContentHeader node={node} />
          <Collapse in={true}>
            <FileLoader node={node} />
          </Collapse>
        </Card>
        <ChangeList node={node} />
        <QuestionList node={node} />
      </Stack>
    </>
  );
};

export default FileApp;
