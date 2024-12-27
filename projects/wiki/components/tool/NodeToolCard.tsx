import { CardActions, Box, Card } from '@mui/material';
import { Edit, People, GetApp } from '@mui/icons-material';
import { Node, useLink } from 'hooks';
import { AutoButton, DeleteButton, PublishButton } from 'comps';
import { useState } from 'react';
import { order_by, resolve } from 'gql';
import HTMLtoDOCX from 'html-to-docx';
import { getLetter } from 'mime';
import { toHtml } from 'core/document';
import { nhost } from 'nhost';

const NodeToolCard = ({ node }: { node: Node }) => {
  const query = node.useQuery();
  const update = node.useUpdate();
  const link = useLink();
  const [openDeleteDialog, setOpenDeleteDialog] = useState(false);
  const id = query?.id;
  const mimeId = query?.mimeId;
  const attachable = query?.attachable;
  const mutable = query?.mutable;

  const handlePublish = () => {
    update({ set: { mutable: false } });
  };

  const handleLockChildren = () => {
    update({ set: { attachable: !attachable } });
  };

  const handleLockContent = () => {
    update({ set: { mutable: !mutable } });
  };

  const handleDelete = () => {
    setOpenDeleteDialog(true);
  };

  const formatContent = async (id: string, level: number): Promise<string> => {
    if (!id) return '';
    const index =
      (
        await resolve(({ query }) =>
          query
            .node({ id })
            ?.parent?.children({
              where: {
                _and: [
                  { mutable: { _eq: false } },
                  { mimeId: { _in: ['vote/change', 'vote/policy'] } },
                ],
              },
              order_by: [{ index: order_by.asc }, { createdAt: order_by.asc }],
            })
            .map(({ id }) => ({ id }))
        )
      )?.findIndex((e) => e.id === id) ?? 0;

    const children = await resolve(({ query }) =>
      query
        .node({ id })
        ?.children({
          order_by: [{ index: order_by.asc }],
          where: {
            mimeId: {
              _in: [
                'vote/position',
                'vote/candidate',
                'vote/policy',
                'vote/change',
              ],
            },
          },
        })
        .map(({ id }) => id)
    );
    const members = await resolve(({ query }) =>
      query
        .node({ id })
        ?.members()
        ?.map((m) => m.name ?? m.user?.displayName)
        .join(', ')
    );
    const node = await resolve(({ query }) => {
      const node = query.node({ id });
      if (node)
        return {
          name: node.name,
          data: node.data(),
          mimeId: node.mimeId,
          context: node.mime?.context,
        };
    });

    const formatedMembers = members?.length
      ? `<i>Stillet af: ${members}</i>`
      : '';

    const prefix =
      node?.mimeId == 'vote/policy'
        ? `${getLetter(index)}: `
        : node?.mimeId == 'vote/change'
        ? `${index + 1}: `
        : '';
    return `<h${level}>${prefix}${
      node?.name
    }</h${level}>${formatedMembers}${toHtml(node?.data?.content)}${
      children
        ? '<br>' +
          (
            await Promise.all(
              children.map((id) => formatContent(id!, level + 1))
            )
          ).join('<br>')
        : ''
    }`;
  };

  const handleDownload = async () => {
    if (!id) return;

    if (mimeId == 'wiki/file') {
      const data = await resolve(({ query }) => query.node({ id })?.data());
      const { presignedUrl } = await nhost.storage.getPresignedUrl({
        fileId: data?.fileId,
      });
      window.open(presignedUrl?.url);
      return;
    }

    const html = await formatContent(id!, 1);

    const blob = await HTMLtoDOCX(html as string, '', {
      table: { row: { cantSplit: true } },
    });

    // Create and evoke link to file
    const blobUrl = URL.createObjectURL(blob as Blob);
    const link = document.createElement('a');
    // eslint-disable-next-line functional/immutable-data
    link.href = blobUrl;
    // eslint-disable-next-line functional/immutable-data
    link.download = `${query?.name}.docx`;
    document.body.appendChild(link);
    link.dispatchEvent(
      new MouseEvent('click', {
        bubbles: true,
        cancelable: true,
        view: window,
      })
    );
    document.body.removeChild(link);
  };

  return (
    <>
      <Card elevation={0}>
        <CardActions>
          <Box sx={{ flexGrow: 1 }} />
          <DeleteButton node={node} />
          {query?.mime?.traits().includes('content') && (
            <AutoButton
              key="edit"
              text="Rediger"
              icon={<Edit />}
              onClick={() => link.push([], 'editor')}
            />
          )}
          <PublishButton node={node} />
        </CardActions>
        <CardActions>
          {query?.isContextOwner && [
            query?.mime?.traits().includes('context') && (
              <AutoButton
                key="member"
                text="Medlemmer"
                icon={<People />}
                onClick={() => link.push([], 'member')}
              />
            ),
          ]}
        </CardActions>
        <CardActions>
          <Box sx={{ flexGrow: 1 }} />
          {query?.isContextOwner && [
            query?.mime?.traits()?.includes('context') && (
              <AutoButton
                key="member"
                text="Medlemmer"
                icon={<People />}
                onClick={() => link.push([], 'member')}
              />
            ),
          ]}
          <AutoButton
            key="download"
            text="Download"
            icon={<GetApp />}
            onClick={handleDownload}
          />
        </CardActions>
      </Card>
    </>
  );
};

export default NodeToolCard;
