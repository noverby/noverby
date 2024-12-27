import React, { startTransition, useEffect, useState } from 'react';
import { Button, Grid, TextField } from '@mui/material';
import { Autocomplete } from '@mui/material';
import { resolve, order_by } from 'gql';
import { Node } from 'hooks';

type Option = {
  name?: string;
  email?: string;
  userId?: string;
  id: string;
};

const InvitesTextField = ({ node }: { node: Node }) => {
  const nodeMembers = node.useMembers();
  const [value, setValue] = useState<Option[]>([]);
  const [options, setOptions] = useState<Option[]>([]);
  const [inputValue, setInputValue] = useState('');

  const handleAddInvites = async () => {
    const members = value.map((user) => ({
      name: user.name,
      email: user.email,
      nodeId: user.userId,
      parentId: node.id,
    }));
    await nodeMembers.insert({ members });
    setValue([]);
  };


  useEffect(() => {
    const fetch = async () => {
      const like = `%${inputValue}%`;
      const users = await resolve(({ query: { users } }) =>
          users({
            limit: 10,
            where: {
              displayName: { _ilike: like },
            },
            order_by: [{ displayName: order_by.asc }],
          })
          .map(({ displayName, id }) => ({ name: displayName, userId: id, id }))
      );

      const newOptions: Option[] = ([] as Option[]).concat(
        users && inputValue.length > 0 ? users : [],
        value ? value : [],
        inputValue ? [{ name: 'N/A', email: inputValue, id: inputValue }] : []
      );

      setOptions(newOptions);
    };
    startTransition(() => {
      fetch();
    });
  }, [JSON.stringify(value), inputValue]);

  return (
    <Grid style={{ margin: 1 }} container spacing={2}>
      <Grid item xs={6}>
        <Autocomplete
          multiple
          color="primary"
          noOptionsText="Ingen match"
          options={options}
          getOptionLabel={(option) => option.email ?? option?.name ?? ''}
          defaultValue={options}
          value={value}
          filterSelectedOptions
          includeInputInList
          autoComplete
          autoHighlight
          onChange={(_, newValue) => {
            setOptions(newValue.concat(options));
            setValue(newValue);
          }}
          onInputChange={(_, newInputValue) => {
            setInputValue(newInputValue);
          }}
          renderInput={(params) => (
            <TextField
              {...params}
              color="primary"
              variant="outlined"
              label="Inviter"
              placeholder="Navn eller Email"
            />
          )}
        />
      </Grid>
      <Grid item xs={6}>
        <Button onClick={handleAddInvites} color="primary" variant="contained">
          Tilføj
        </Button>
      </Grid>
    </Grid>
  );
};

export default InvitesTextField;
