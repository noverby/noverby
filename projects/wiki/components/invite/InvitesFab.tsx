import React from 'react';
import { Fab } from '@mui/material';
import { GroupAdd } from '@mui/icons-material';
import { SheetReader } from 'comps';
import { Node } from 'hooks';

const InvitesFab = ({ node }: { node: Node }) => {
  const parentId = node.id;
  const nodeMembers = node.useMembers();
  const handleFile = async (
    fileData: { Fornavn: string; Efternavn: string; Email: string }[]
  ) => {
    const members = fileData
      .filter((r) => r?.Email)
      .map((r) => ({
        name: `${r.Fornavn} ${r.Efternavn}`,
        email: r?.Email?.toLowerCase(),
        parentId,
      }));
    await nodeMembers.insert({ members });
  };

  const parseOptions = {
    header: true,
    dynamicTyping: true,
    skipEmptyLines: true,
    transformHeader: (header: string) =>
      header.toLowerCase().replace(/\W/g, '_'),
  };

  return (
    <SheetReader onFileLoaded={handleFile}>
      <Fab
        sx={{
          position: 'fixed',
          bottom: (t) => t.spacing(9),
          right: (t) => t.spacing(3),
        }}
        variant="extended"
        color="primary"
        aria-label="Tilføj adgang"
        component="span"
      >
        <GroupAdd />
        import
      </Fab>
    </SheetReader>
  );
}

export default InvitesFab;